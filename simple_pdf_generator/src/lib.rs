use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose;
use base64::Engine;

use headless_chrome::{types::PrintToPdfOptions, Browser, LaunchOptions, Tab};

use anyhow::Result;
use futures::future::join_all;
use rayon::prelude::ParallelIterator;
use rayon::str::ParallelString;
use regex::Regex;

use once_cell::sync::Lazy;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum AssetType {
    Style,
    Script,
}

#[derive(Debug, Default)]
pub struct Template {
    pub html_path: PathBuf,
    pub properties: HashMap<String, Property>,
    pub tables: HashMap<String, String>,
}

#[derive(Debug)]
pub struct Property {
    pub val: String,
    pub is_none: bool,
    pub is_tabledata: bool,
}

#[derive(Debug)]
pub struct Asset {
    pub path: PathBuf,
    pub r#type: AssetType,
}

static BROWSER: Lazy<RwLock<Browser>> = Lazy::new(|| {
    let options = LaunchOptions::default_builder()
        .headless(true)
        .build()
        .expect("Couldn't find appropriate Chrome binary.");
    let browser = Browser::new(options).expect("Couldn't create browser.");

    RwLock::new(browser)
});

static TOKENS_AND_IMAGES_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:%%(?P<prop_name>.*)%%)|(?:<img[^>]*\ssrc="(?P<img_src>.*?)"[^>]*>)"#).unwrap()
});

pub async fn generate_pdf(template: Template, assets: &[Asset]) -> Result<Vec<u8>> {
    let html = tokio::fs::read_to_string(template.html_path.clone()).await?;

    let mut xpath_texts: Vec<String> = Vec::new();

    let html = TOKENS_AND_IMAGES_REGEX
        .replace_all(&html, |caps: &regex::Captures| {
            let prop_name = caps.name("prop_name").map(|prop_name| prop_name.as_str());
            let img_src = caps.name("img_src").map(|img_src| img_src.as_str());
            let mut result = String::new();

            if let Some(prop_name) = prop_name {
                if let Some(value) = template.properties.get(prop_name) {
                    if value.is_none {
                        xpath_texts.push(format!("text() = '{}'", prop_name));
                        result = prop_name.to_string();
                    } else {
                        result = value.val.clone();
                    }
                }
            } else if let Some(img_src) = img_src {
                if img_src.starts_with("data:image") {
                    result = img_src.to_string();
                } else {
                    let mime_type = mime_guess::from_path(img_src).first_raw();
                    if let Some(mime_type) = mime_type {
                        let mut img_src_path = Path::new(img_src).to_owned();
                        if img_src_path.is_relative() {
                            img_src_path = template
                                .html_path
                                .parent()
                                .unwrap_or_else(|| Path::new(""))
                                .join(img_src_path)
                                .canonicalize()
                                .unwrap_or_else(|_| PathBuf::new());
                        }

                        let img_data = fs::read(img_src_path).unwrap_or(Vec::new());
                        let image_base64 = general_purpose::STANDARD.encode(img_data);
                        let new_src = format!("data:{};base64,{}", mime_type, image_base64);

                        result = caps.get(0).unwrap().as_str().replace(img_src, &new_src);
                    } else {
                        result = img_src.to_string();
                    }
                }
            }

            result
        })
        .par_chars()
        .collect::<String>();

    let html = urlencoding::encode(&html).to_string();
    let browser_lock = BROWSER.read().await;
    let tab = match browser_lock.new_tab() {
        Ok(tab) => {
            drop(browser_lock);
            tab
        }
        Err(_) => {
            drop(browser_lock);

            let mut browser = BROWSER.write().await;
            let options = LaunchOptions::default_builder()
                .headless(true)
    let tab = tokio::task::spawn_blocking(move || {
        let tab = BROWSER.new_tab().unwrap();
        tab.navigate_to(&format!("data:text/html,{}", html))
            .unwrap()
            .wait_until_navigated()
            .unwrap();
                .build()
                .expect("Couldn't find appropriate Chrome binary.");
            *browser = Browser::new(options).expect("Couldn't create browser.");
            browser.new_tab()?
        }
    };

    tab.navigate_to(&format!("data:text/html,{}", html))
        .unwrap()
        .wait_until_navigated()
        .unwrap();

    let mut asset_content_futures = Vec::new();
    for asset in assets {
        asset_content_futures.push(tokio::fs::read_to_string(asset.path.clone()));
    }

    let asset_contents = join_all(asset_content_futures).await;
    let mut inject_futures = Vec::new();
    for (index, asset_content) in asset_contents.into_iter().enumerate() {
        let Ok(asset_data) = asset_content else {
            continue;
        };
        let asset = &assets[index];
        let tab = tab.clone();

        match asset.r#type {
            AssetType::Style => {
                inject_futures.push(tokio::task::spawn_blocking(move || {
                    inject_css(&tab, &asset_data)
                }));
            }
            AssetType::Script => {
                inject_futures.push(tokio::task::spawn_blocking(move || {
                    inject_js(&tab, &asset_data)
                }));
            }
        }
    }

    let _ = join_all(inject_futures).await;

    if !template.tables.is_empty() {
        let table_generator_js: &'static str = include_str!("../assets/js/table-generator.js");

        let mut tables_data = "tablesData = {".to_string();
        for (table_name, mut table_data) in template.tables {
            if table_data.is_empty() {
                table_data = "[]".to_string();
                xpath_texts.push(format!("@items = '{}'", table_name));
            }

            tables_data.push_str(&format!("{}:{},", table_name, table_data));
        }
        tables_data.push('}');

        let table_generator_js = table_generator_js.replacen("tablesData", &tables_data, 1);
        let tab = tab.clone();
        _ = tokio::task::spawn_blocking(move || tab.evaluate(&table_generator_js, false)).await?;
    }

    if !xpath_texts.is_empty() {
        let xpath_expression = format!(
            "//*[not(self::script or self::style or self::title) and ({})]",
            xpath_texts.join(" or ")
        );
        let js_script = format!(
            "
            function hideNoneElements() {{
                const xpathExpression = `{}`;
                const result = document.evaluate(xpathExpression, document, null, XPathResult.UNORDERED_NODE_SNAPSHOT_TYPE, null);

                for (let i = 0; i < result.snapshotLength; i++) {{
                    const targetElement = result.snapshotItem(i);
                    targetElement.style.display = 'none';
                }}
            }}
            hideNoneElements();
            ",
            xpath_expression
        );

        let tab = tab.clone();
        _ = tokio::task::spawn_blocking(move || tab.evaluate(&js_script, false)).await?
    }

    let print_options = PrintToPdfOptions {
        print_background: Some(true),
        ..Default::default()
    };

    tokio::task::spawn_blocking(move || tab.print_to_pdf(Some(print_options))).await?
}

fn inject_js(tab: &Tab, js: &str) -> Result<()> {
    tab.wait_for_element("head")?.call_js_fn(
        "function() { 
            const script = document.createElement('script');
            script.innerHTML = arguments[0];
            document.head.appendChild(script);
        }",
        vec![serde_json::Value::String(js.to_string())],
        false,
    )?;

    Ok(())
}

fn inject_css(tab: &Tab, css: &str) -> Result<()> {
    tab.wait_for_element("head")?.call_js_fn(
        "function() { 
            const style = document.createElement('style');
            style.innerHTML = arguments[0];
            document.head.appendChild(style);
        }",
        vec![serde_json::Value::String(css.to_string())],
        false,
    )?;

    Ok(())
}
