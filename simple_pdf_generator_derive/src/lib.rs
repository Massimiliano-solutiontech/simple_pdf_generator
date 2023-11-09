use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(PdfTemplate, attributes(PdfTableData))]
pub fn pdf_template_property(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let struct_fields = match input.data {
        Data::Struct(ref data) => &data.fields,
        _ => panic!("PdfTemplate can only be derived for structs"),
    };

    let inspect_struct_fields = struct_fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_ty = &field.ty;

        let is_tabledata = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("PdfTableData"));

        if is_tabledata {
            quote! {
                template.tables.insert(
                    stringify!(#field_name).to_string(),
                    stringify_object(&self.#field_name),
                );
            }
        } else {
            let property = match field_ty {
                syn::Type::Path(type_path) => {
                    let type_name = type_path.path.segments.first().unwrap().ident.to_string();
                    if type_name == "Option" {
                        quote! {
                            simple_pdf_generator::Property {
                                val: match &self.#field_name {
                                    std::option::Option::Some(value) => value.to_string(),
                                    std::option::Option::None => String::new(),
                                },
                                is_none: self.#field_name.is_none(),
                                is_tabledata: false,
                            }
                        }
                    } else {
                        quote! {
                            simple_pdf_generator::Property {
                                val: self.#field_name.to_string(),
                                is_none: false,
                                is_tabledata: false,
                            }
                        }
                    }
                }
                _ => quote! {
                    simple_pdf_generator::Property {
                        val: self.#field_name.to_string(),
                        is_none: false,
                        is_tabledata: false,
                    }
                },
            };

            quote! {
                template.properties.insert(
                    stringify!(#field_name).to_string(),
                    #property,
                );
            }
        }
    });

    let impl_methods = quote! {
        impl #struct_name {
            pub async fn generate_pdf(&self,
                html_path: std::path::PathBuf,
                assets: &[simple_pdf_generator::Asset],
                print_options: &simple_pdf_generator::PrintOptions,
            ) -> std::result::Result<Vec<u8>, simple_pdf_generator::SimplePdfGeneratorError> {
                let mut template = simple_pdf_generator::Template::default();
                template.html_path = html_path;
                #(#inspect_struct_fields)*

                simple_pdf_generator::generate_pdf(template, assets, print_options).await
            }
        }
    };

    let utility_methods = quote! {
        fn stringify_object<T: serde::Serialize>(obj: &T) -> String {
            let mut result = String::new();

            let serialized = serde_json::to_value(obj).unwrap();
            if let serde_json::Value::Object(map) = &serialized {
                result.push('{');
                for (key, value) in map {
                    result.push_str(&format!("{}:{},", key, value));
                }
                result.push('}');
            } else if let serde_json::Value::Array(array) = serialized {
                result.push('[');
                for value in array {
                    result.push_str(&format!("{},", value));
                }
                result.push(']');
            }

            result
        }
    };

    quote! {
        #impl_methods
        #utility_methods
    }
    .into()
}

#[proc_macro_derive(PdfTemplateForHtml, attributes(PdfTableData))]
pub fn pdf_template_property_for_html_string(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let struct_fields = match input.data {
        Data::Struct(ref data) => &data.fields,
        _ => panic!("PdfTemplateForHtml can only be derived for structs"),
    };

    let inspect_struct_fields = struct_fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_ty = &field.ty;

        let is_tabledata = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("PdfTableData"));

        if is_tabledata {
            quote! {
                template.tables.insert(
                    stringify!(#field_name).to_string(),
                    stringify_object(&self.#field_name),
                );
            }
        } else {
            let property = match field_ty {
                syn::Type::Path(type_path) => {
                    let type_name = type_path.path.segments.first().unwrap().ident.to_string();
                    if type_name == "Option" {
                        quote! {
                            simple_pdf_generator::Property {
                                val: match &self.#field_name {
                                    std::option::Option::Some(value) => value.to_string(),
                                    std::option::Option::None => String::new(),
                                },
                                is_none: self.#field_name.is_none(),
                                is_tabledata: false,
                            }
                        }
                    } else {
                        quote! {
                            simple_pdf_generator::Property {
                                val: self.#field_name.to_string(),
                                is_none: false,
                                is_tabledata: false,
                            }
                        }
                    }
                }
                _ => quote! {
                    simple_pdf_generator::Property {
                        val: self.#field_name.to_string(),
                        is_none: false,
                        is_tabledata: false,
                    }
                },
            };

            quote! {
                template.properties.insert(
                    stringify!(#field_name).to_string(),
                    #property,
                );
            }
        }
    });

    let impl_methods = quote! {
        impl #struct_name {
            pub async fn generate_pdf_from_html(&self,
              html_string: String,
              attributes: 
              assets: &[simple_pdf_generator::Asset],
              print_options: &simple_pdf_generator::PrintOptions,
          ) -> std::result::Result<Vec<u8>, simple_pdf_generator::SimplePdfGeneratorError> {
              let mut template = simple_pdf_generator::Template::default();

              #(#inspect_struct_fields)*
              simple_pdf_generator::generate_pdf_from_html(html_string, template, assets, print_options).await
          }
        }
    };

    let utility_methods = quote! {
        fn stringify_object<T: serde::Serialize>(obj: &T) -> String {
            let mut result = String::new();

            let serialized = serde_json::to_value(obj).unwrap();
            if let serde_json::Value::Object(map) = &serialized {
                result.push('{');
                for (key, value) in map {
                    result.push_str(&format!("{}:{},", key, value));
                }
                result.push('}');
            } else if let serde_json::Value::Array(array) = serialized {
                result.push('[');
                for value in array {
                    result.push_str(&format!("{},", value));
                }
                result.push(']');
            }

            result
        }
    };

    quote! {
        #impl_methods
        #utility_methods
    }
    .into()
}
