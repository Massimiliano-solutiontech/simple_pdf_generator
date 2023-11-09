use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use base64::engine::general_purpose;
use base64::Engine;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use chromiumoxide::error::CdpError;
use chromiumoxide::js::EvaluationResult;
use chromiumoxide::Page;
use futures::future::try_join_all;
use futures::StreamExt;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum AssetType {
    Style,
    Script,
}

#[derive(Debug)]
pub enum SimplePdfGeneratorError {
    BrowserError(String),
    IoError(String),
    PdfError(String),
}

impl Display for SimplePdfGeneratorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SimplePdfGeneratorError::BrowserError(msg) => {
                write!(f, "Browser error: {}", msg)
            }
            SimplePdfGeneratorError::IoError(msg) => write!(f, "IO error: {}", msg),
            SimplePdfGeneratorError::PdfError(msg) => write!(f, "PDF error: {}", msg),
        }
    }
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

pub struct PrintOptions {
    pub print_background: bool,
    pub paper_width: Option<f64>,
    pub paper_height: Option<f64>,
    pub margin_top: Option<f64>,
    pub margin_bottom: Option<f64>,
    pub margin_left: Option<f64>,
    pub margin_right: Option<f64>,
    pub page_ranges: Option<String>,
    pub prefer_css_page_size: bool,
    pub landscape: bool,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            print_background: true,
            paper_width: None,
            paper_height: None,
            margin_top: Some(0.0),
            margin_bottom: Some(0.0),
            margin_left: Some(0.0),
            margin_right: Some(0.0),
            page_ranges: None,
            prefer_css_page_size: false,
            landscape: false,
        }
    }
}

impl From<&PrintOptions> for PrintToPdfParams {
    fn from(val: &PrintOptions) -> Self {
        PrintToPdfParams {
            print_background: Some(val.print_background),
            paper_width: val.paper_width.map(|val| val / 25.4),
            paper_height: val.paper_height.map(|val| val / 25.4),
            margin_top: val.margin_top.map(|val| val / 25.4),
            margin_bottom: val.margin_bottom.map(|val| val / 25.4),
            margin_left: val.margin_left.map(|val| val / 25.4),
            margin_right: val.margin_right.map(|val| val / 25.4),
            landscape: Some(val.landscape),
            ..Default::default()
        }
    }
}

struct ChromiumInstance {
    browser: Browser,
}

impl ChromiumInstance {
    async fn new() -> Self {
        let options = BrowserConfig::builder();
        let options = if NO_SANDBOX.load(Ordering::Relaxed) {
            options.no_sandbox()
        } else {
            options
        };
        let options = options.build().expect("Invalid browser options.");

        let (browser, mut handler) = Browser::launch(options)
            .await
            .expect("Couldn't create browser.");

        tokio::task::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }

            let write_guard = BROWSER.try_write();
            if let Ok(mut guard) = write_guard {
                *guard = None;
            }
        });

        ChromiumInstance { browser }
    }
}

static BROWSER: Lazy<RwLock<Option<ChromiumInstance>>> = Lazy::new(|| RwLock::new(None));
static TOKENS_AND_IMAGES_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:%%(?P<prop_name>.*)%%)|(?:<img[^>]*\ssrc="(?P<img_src>.*?)"[^>]*>)"#).unwrap()
});
static NO_SANDBOX: AtomicBool = AtomicBool::new(false);

pub fn set_no_sandbox(val: bool) {
    NO_SANDBOX.store(val, Ordering::Relaxed);
}

pub async fn generate_pdf_from_html(
    html: String,
    assets: &[Asset],
    print_options: &PrintOptions,
) -> Result<Vec<u8>, SimplePdfGeneratorError> {
    let template = Template::default();
    let mut xpath_texts: Vec<String> = Vec::new();
    let html = TOKENS_AND_IMAGES_REGEX
        .replace_all(&html, |caps: &regex::Captures| {
            let prop_name = caps.name("prop_name").map(|prop_name| prop_name.as_str());
            let img_src = caps.name("img_src").map(|img_src| img_src.as_str());
            let mut result = String::new();

            if let Some(prop_name) = prop_name {
              if let Some(property) = template.properties.get(prop_name) {
                  if property.is_none {
                      xpath_texts.push(format!("text() = '{}'", prop_name));
                      result = prop_name.to_string();
                  } else {
                      result = html_escape::encode_text(&property.val).to_string()
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
      .to_string();

    let browser = get_browser().await;
    let browser_instance = browser
        .as_ref()
        .ok_or(SimplePdfGeneratorError::BrowserError(
            "Cannot create the browser".to_string(),
        ))?;
    let page = browser_instance
        .browser
        .new_page("about:blank")
        .await
        .map_err(|e| {
            SimplePdfGeneratorError::BrowserError(format!("Cannot create the page: {}", e))
        })?;
    page.set_content(html).await.map_err(|e| {
        SimplePdfGeneratorError::BrowserError(format!("Cannot set the content: {}", e))
    })?;

    let mut asset_content_futures = Vec::new();
    for asset in assets {
        asset_content_futures.push(tokio::fs::read_to_string(asset.path.clone()));
    }

    let asset_contents = try_join_all(asset_content_futures)
        .await
        .map_err(|e| SimplePdfGeneratorError::IoError(format!("Cannot read the asset: {}", e)))?;
    let mut inject_futures_css = Vec::new();
    let mut inject_futures_js = Vec::new();
    for (index, asset_content) in asset_contents.into_iter().enumerate() {
        match assets[index].r#type {
            AssetType::Style => {
                inject_futures_css.push(inject_css(&page, asset_content));
            }
            AssetType::Script => {
                inject_futures_js.push(inject_js(&page, asset_content));
            }
        }
    }
    try_join_all(inject_futures_css).await.map_err(|e| {
        SimplePdfGeneratorError::BrowserError(format!("Cannot inject the css: {}", e))
    })?;
    try_join_all(inject_futures_js).await.map_err(|e| {
        SimplePdfGeneratorError::BrowserError(format!("Cannot inject the js: {}", e))
    })?;

    if !xpath_texts.is_empty() {
        let xpath_expression = format!(
            "//*[not(self::script or self::style or self::title) and ({})]",
            xpath_texts.join(" or ")
        );
        let js_script = format!(
            "
            () => {{
                const xpathExpression = `{}`;
                const result = document.evaluate(xpathExpression, document, null, XPathResult.UNORDERED_NODE_SNAPSHOT_TYPE, null);

                for (let i = 0; i < result.snapshotLength; i++) {{
                    const targetElement = result.snapshotItem(i);
                    targetElement.style.display = 'none';
                }}
            }}
            ",
            xpath_expression
        );

        _ = page.evaluate(js_script).await.map_err(|e| {
            SimplePdfGeneratorError::BrowserError(format!(
                "Cannot evaluate the xPath script: {}",
                e
            ))
        })?;
    }

    page.pdf(print_options.into())
        .await
        .map_err(|e| SimplePdfGeneratorError::PdfError(format!("Cannot create the pdf: {}", e)))
}

pub async fn generate_pdf(
  template: Template,
  assets: &[Asset],
  print_options: &PrintOptions,
) -> Result<Vec<u8>, SimplePdfGeneratorError> {
  let html = tokio::fs::read_to_string(template.html_path.clone())
      .await
      .map_err(|e| {
          SimplePdfGeneratorError::IoError(format!("Cannot read the html file: {}", e))
      })?;

  let mut xpath_texts: Vec<String> = Vec::new();
  let html = TOKENS_AND_IMAGES_REGEX
      .replace_all(&html, |caps: &regex::Captures| {
          let prop_name = caps.name("prop_name").map(|prop_name| prop_name.as_str());
          let img_src = caps.name("img_src").map(|img_src| img_src.as_str());
          let mut result = String::new();

          if let Some(prop_name) = prop_name {
              if let Some(property) = template.properties.get(prop_name) {
                  if property.is_none {
                      xpath_texts.push(format!("text() = '{}'", prop_name));
                      result = prop_name.to_string();
                  } else {
                      result = html_escape::encode_text(&property.val).to_string()
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
      .to_string();

  let browser = get_browser().await;
  let browser_instance = browser
      .as_ref()
      .ok_or(SimplePdfGeneratorError::BrowserError(
          "Cannot create the browser".to_string(),
      ))?;
  let page = browser_instance
      .browser
      .new_page("about:blank")
      .await
      .map_err(|e| {
          SimplePdfGeneratorError::BrowserError(format!("Cannot create the page: {}", e))
      })?;
  page.set_content(html).await.map_err(|e| {
      SimplePdfGeneratorError::BrowserError(format!("Cannot set the content: {}", e))
  })?;

  let mut asset_content_futures = Vec::new();
  for asset in assets {
      asset_content_futures.push(tokio::fs::read_to_string(asset.path.clone()));
  }

  let asset_contents = try_join_all(asset_content_futures)
      .await
      .map_err(|e| SimplePdfGeneratorError::IoError(format!("Cannot read the asset: {}", e)))?;
  let mut inject_futures_css = Vec::new();
  let mut inject_futures_js = Vec::new();
  for (index, asset_content) in asset_contents.into_iter().enumerate() {
      match assets[index].r#type {
          AssetType::Style => {
              inject_futures_css.push(inject_css(&page, asset_content));
          }
          AssetType::Script => {
              inject_futures_js.push(inject_js(&page, asset_content));
          }
      }
  }
  try_join_all(inject_futures_css).await.map_err(|e| {
      SimplePdfGeneratorError::BrowserError(format!("Cannot inject the css: {}", e))
  })?;
  try_join_all(inject_futures_js).await.map_err(|e| {
      SimplePdfGeneratorError::BrowserError(format!("Cannot inject the js: {}", e))
  })?;

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

      let table_generator_js =
          table_generator_js.replacen("tablesData", &html_escape::encode_text(&tables_data), 1);
      _ = page.evaluate(table_generator_js).await.map_err(|e| {
          SimplePdfGeneratorError::BrowserError(format!("Cannot evaluate the js: {}", e))
      })?;
  }

  if !xpath_texts.is_empty() {
      let xpath_expression = format!(
          "//*[not(self::script or self::style or self::title) and ({})]",
          xpath_texts.join(" or ")
      );
      let js_script = format!(
          "
          () => {{
              const xpathExpression = `{}`;
              const result = document.evaluate(xpathExpression, document, null, XPathResult.UNORDERED_NODE_SNAPSHOT_TYPE, null);

              for (let i = 0; i < result.snapshotLength; i++) {{
                  const targetElement = result.snapshotItem(i);
                  targetElement.style.display = 'none';
              }}
          }}
          ",
          xpath_expression
      );

      _ = page.evaluate(js_script).await.map_err(|e| {
          SimplePdfGeneratorError::BrowserError(format!(
              "Cannot evaluate the xPath script: {}",
              e
          ))
      })?;
  }

  page.pdf(print_options.into())
      .await
      .map_err(|e| SimplePdfGeneratorError::PdfError(format!("Cannot create the pdf: {}", e)))
}

async fn inject_js(page: &Page, js: String) -> Result<EvaluationResult, CdpError> {
    let script = format!(
        "() => {{ 
            const script = document.createElement('script');
            script.innerHTML = `{}`;
            document.head.appendChild(script);
    }}",
        js
    );

    page.evaluate(script).await
}

async fn inject_css(page: &Page, css: String) -> Result<EvaluationResult, CdpError> {
    let script = format!(
        "() => {{ 
            const style = document.createElement('style');
            style.innerHTML = `{}`;
            document.head.appendChild(style);
    }}",
        css
    );

    page.evaluate(script).await
}

async fn get_browser<'a>() -> tokio::sync::RwLockReadGuard<'a, Option<ChromiumInstance>> {
    let read_guard = BROWSER.read().await;
    if read_guard.is_some() {
        return read_guard;
    }

    drop(read_guard);

    let mut write_guard = BROWSER.write().await;

    if write_guard.is_none() {
        *write_guard = Some(ChromiumInstance::new().await);
    }

    drop(write_guard);
    BROWSER.read().await
}
