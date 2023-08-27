# Simple PDF Generator

Rust library that converts HTML5 files to PDFs. That's the Rust version of [simple-pdf-generator](https://github.com/lorenzinigiovanni/simple-pdf-generator)

## Installation

This is a Rust library available through [crates.io](https://crates.io/crates/simple_pdf_generator).
Before installing, download and install Node.js.

Installation is done using cargo add command:

```sh
$ cargo add simple_pdf_generator
```

## Features

Simple PDF Generator:

-   supports `Option` types;
-   supports custom CSS and JS;
-   fills custom fields in the HTML template;
-   can generate dynamic tables automatically.

## Quick Start

In order to have a template you must create struct with `PdfTemplate` derive:

```rust
use simple_pdf_generator::{Asset, AssetType};
use simple_pdf_generator_derive::PdfTemplate;

struct Example {
    id: i64,
    name: Option<String>,
    opt_value: Option<String>,
    surname: String,
    is_true: bool,
}
```

And add the HTML file:

```html
<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title></title>
    </head>

    <body>
        <div class="container">
            <div class="row">
                <div class="col">Id</div>
                <div class="col">%%id%%</div>

                <div class="col">Name</div>
                <div class="col">%%name%%</div>

                <div class="col">Opt Value</div>
                <div class="col">%%opt_value%%</div>

                <div class="col">Surname</div>
                <div class="col">%%surname%%</div>

                <div class="col">Is True</div>
                <div class="col">%%is_true%%</div>
            </div>
        </div>
    </body>
</html>
```

Now you can use the `Example` struct to generate the PDF file:

```rust
use std::env;

use simple_pdf_generator::{Asset, AssetType};
use simple_pdf_generator_derive::PdfTemplate;

#[tokio::main]
async fn main() {
    // fill the struct
    let example = Example {
        id: 1,
        name: Some("Foo".to_string()),
        opt_value: None,
        surname: "Bar".to_string(),
        is_true: true,
    };

    // get the html template path
    let html_path = env::current_dir()
        .unwrap()
        .join("test_suite")
        .join("src/template/index.html");

    // inject some assets, in this case the bootstrap css
    let assets = [Asset {
        path: env::current_dir()
            .unwrap()
            .join("test_suite")
            .join("src/template/css/bootstrap.min.css"),
        r#type: AssetType::Style,
    }];

    // generate the pdf file
    let pdf_buf = example.generate_pdf(html_path, &assets).await;
    tokio::fs::write("example.pdf", pdf_buf).await.expect("Unable to write file");
}
```

## Generate Tables

To generate a table you must to use the `PdfTableData` macro on your data property:

```rust
use std::env;

use serde::Serialize;
use simple_pdf_generator::{Asset, AssetType};
use simple_pdf_generator_derive::PdfTemplate;

struct Example {
    id: i64,
    name: Option<String>,
    opt_value: Option<String>,
    surname: String,
    is_true: bool,
    #[PdfTableData]
    my_table: Vec<MyTableData>,
}

#[derive(Serialize)]
struct MyTableData {
    index: u8,
    name: String,
    surname: String,
    email: String,
}

#[tokio::main]
async fn main() {
    // fill the struct
    let example = Example {
        id: 1,
        name: Some("Foo".to_string()),
        opt_value: None,
        surname: "Bar".to_string(),
        is_true: true,
        my_table: vec![
            MyTableData {
                index: 1,
                name: "James".to_string(),
                surname: "Smith".to_string(),
                email: "james.smith@evilcorp.tech".to_string(),
            },
            MyTableData {
                index: 2,
                name: "Robert".to_string(),
                surname: "Johnson".to_string(),
                email: "robert.johnson@evilcorp.tech".to_string(),
            },
        ],
    };

    // get the html template path
    let html_path = env::current_dir()
        .unwrap()
        .join("test_suite")
        .join("src/template/index.html");

    // inject some assets, in this case the bootstrap css
    let assets = [Asset {
        path: env::current_dir()
            .unwrap()
            .join("test_suite")
            .join("src/template/css/bootstrap.min.css"),
        r#type: AssetType::Style,
    }];

    // generate the pdf file
    let pdf_buf = example.generate_pdf(html_path, &assets).await;
    tokio::fs::write("example.pdf", pdf_buf).await.expect("Unable to write file");
}
```

In the HTML file write this:

```html
<inject-table items="my_table">
    <inject-column prop="index" label="#" />
    <inject-column prop="name" label="Name" />
    <inject-column prop="surname" label="Surname" />
    <inject-column prop="email" label="Email" />
</inject-table>
```

## API

### Macros

| Macro          | Purpose                                                                        | HTML use                                                                                                                                        |
| -------------- | ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `PdfTemplate`  | Define the struct that contains the values for the HTML placeholders           | `%%prop_name%%`                                                                                                                                 |
| `PdfTableData` | Define the data for the table. Must be a `vector` of `serde::Serialize` struct | `<inject-table items="prop_name">`<br>&nbsp;&nbsp;&nbsp;&nbsp;`<inject-column prop="struct_prop_name" label="Something"/>`<br>`</inject-table>` |

### `generate_pdf`

`generate_pdf(html_path: std::path::PathBuf, assets: &[simple_pdf_generator::Asset]) -> Result<Vec<u8>, Box<dyn std::error::Error>>` returns the PDF file as a `Vec<u8>`.

| Parameter                                | Description                         |
| ---------------------------------------- | ----------------------------------- |
| `html_path: std::path::PathBuf`          | PDF output dir                      |
| `assets: &[simple_pdf_generator::Asset]` | Object with `Puppeteer PDF Options` |

### `simple_pdf_generator::{Asset, AssetType}`

```rust
struct Asset {
    pub path: PathBuf,
    pub r#type: AssetType,
}

enum AssetType {
    Style,
    Script,
}
```

## People

This library is developed by:

-   [@MassimilianoMontagni](https://github.com/Massimiliano-solutiontech) [solutiontech.tech](https://www.solutiontech.tech/)

## License

[MIT](LICENSE)
