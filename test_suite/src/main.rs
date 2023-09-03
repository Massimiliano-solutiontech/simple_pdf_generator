use std::{env, time};

use futures::future::join_all;
use serde::Serialize;
use simple_pdf_generator::{Asset, AssetType, PrintOptions};
use simple_pdf_generator_derive::PdfTemplate;

#[derive(PdfTemplate)]
struct Example {
    id: i64,
    name: Option<String>,
    opt_value: Option<String>,
    surname: String,
    is_true: bool,
    #[PdfTableData]
    data: Vec<JsonStruct>,
    #[PdfTableData]
    another_table: Vec<AnotherTableJsonStruct>,
    #[PdfTableData]
    nullable_table: Option<Vec<AnotherTableJsonStruct>>,
}

#[derive(Serialize)]
struct JsonStruct {
    index: u8,
    name: String,
    surname: String,
    email: String,
}

#[derive(Serialize)]
struct AnotherTableJsonStruct {
    name: String,
    surname: String,
}

#[tokio::main]
async fn main() {
    let test = Example {
        id: 1,
        name: None,
        opt_value: None,
        surname: "test2".to_string(),
        is_true: true,
        data: vec![
            JsonStruct {
                index: 1,
                name: "test".to_string(),
                surname: "test2".to_string(),
                email: "".to_string(),
            },
            JsonStruct {
                index: 2,
                name: "test1".to_string(),
                surname: "test4".to_string(),
                email: "ciro@ciro.it".to_string(),
            },
        ],
        another_table: vec![
            AnotherTableJsonStruct {
                name: "Mario".to_string(),
                surname: "Rossi".to_string(),
            },
            AnotherTableJsonStruct {
                name: "Ciro".to_string(),
                surname: "Esposito".to_string(),
            },
        ],
        nullable_table: None,
    };

    let html_path = env::current_dir()
        .unwrap()
        .join("test_suite")
        .join("src/template/index.html");

    let assets = [Asset {
        path: env::current_dir()
            .unwrap()
            .join("test_suite")
            .join("src/template/css/bootstrap.min.css"),
        r#type: AssetType::Style,
    }];

    let print_options = PrintOptions {
        paper_width: Some(210.0),
        paper_height: Some(297.0),
        margin_top: Some(10.0),
        margin_bottom: Some(10.0),
        margin_left: Some(10.0),
        margin_right: Some(10.0),
        ..PrintOptions::default()
    };

    let start = time::Instant::now();
    _ = test
        .generate_pdf(html_path.clone(), &assets, &print_options)
        .await;
    let duration = start.elapsed();
    println!("single completed in {:?}", duration);

    let start = time::Instant::now();

    let gen_0 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_1 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_2 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_3 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_4 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_5 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_6 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_7 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_8 = test.generate_pdf(html_path.clone(), &assets, &print_options);
    let gen_9 = test.generate_pdf(html_path.clone(), &assets, &print_options);

    let futures_res = join_all(vec![
        gen_0, gen_1, gen_2, gen_3, gen_4, gen_5, gen_6, gen_7, gen_8, gen_9,
    ])
    .await;

    let duration = start.elapsed();
    println!("completed in {:?}", duration);

    for res in futures_res.iter().enumerate() {
        let Ok(content) = res.1.as_ref() else {
            println!("Error on {} {}", res.0, res.1.as_ref().unwrap_err());
            continue;
        };

        _ = tokio::fs::write(format!("result-{}.pdf", res.0), content).await;
    }
}
