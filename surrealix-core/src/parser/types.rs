use super::QueryType;
use convert_case::{Case, Casing};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use surrealdb::sql::Kind;

pub fn generate_query_types(query_type: &QueryType, struct_name: &str) -> TokenStream {
    println!("Generating query types for: {:?}", query_type);
    let struct_name_ident = format_ident!("{}", struct_name);

    match query_type {
        QueryType::Object(fields) => {
            println!("Generating struct for object");
            let fields = fields.iter().map(|(name, field_type)| {
                let field_name = format_ident!("{}", name);
                let field_type = generate_type_for_value(field_type, name);
                quote! { pub #field_name: #field_type }
            });

            quote! {
                pub struct #struct_name_ident {
                    #(#fields),*
                }
            }
        }
        QueryType::Array(inner) => {
            println!("Generating type for array");
            let inner_type = generate_type_for_value(inner, "");
            quote! {
                pub type #struct_name_ident = Vec<#inner_type>;
            }
        }
        QueryType::Value(kind) => {
            println!("Generating type for value");
            let type_tokens = kind_to_type(kind);
            quote! {
                pub type #struct_name_ident = #type_tokens;
            }
        }
    }
}

fn generate_type_for_value(qt: &QueryType, name: &str) -> TokenStream {
    match qt {
        QueryType::Object(_) => {
            let struct_name = format_ident!("{}", name.to_case(Case::Pascal));
            generate_struct(qt, &struct_name.to_string());
            quote! { #struct_name }
        }
        QueryType::Value(kind) => kind_to_type(kind),
        QueryType::Array(inner) => {
            let inner_type = generate_type_for_value(inner, name);
            quote! { Vec<#inner_type> }
        }
    }
}

fn generate_struct(qt: &QueryType, name: &str) -> TokenStream {
    match qt {
        QueryType::Object(fields) => {
            let struct_name = format_ident!("{}", name.to_case(Case::Pascal));
            let mut field_tokens = TokenStream::new();
            let mut additional_types = TokenStream::new();

            for (field_name, field_type) in fields {
                let field_ident = format_ident!("{}", field_name.to_case(Case::Snake));
                let field_type_tokens = match field_type {
                    QueryType::Value(kind) => kind_to_type(kind),
                    QueryType::Array(inner) => match &**inner {
                        QueryType::Value(Kind::Any) => quote! { Vec<serde_json::Value> },
                        QueryType::Value(kind) => {
                            let inner_type = kind_to_type(kind);
                            quote! { Vec<#inner_type> }
                        }
                        QueryType::Object(_) => {
                            let inner_name = format!(
                                "{}{}",
                                name.to_case(Case::Pascal),
                                field_name.to_case(Case::Pascal)
                            );
                            let inner_struct = generate_struct(inner, &inner_name);
                            additional_types.extend(inner_struct);
                            let inner_type = format_ident!("{}", inner_name);
                            quote! { Vec<#inner_type> }
                        }
                        QueryType::Array(_) => quote! { Vec<Vec<serde_json::Value>> },
                    },
                    QueryType::Object(_) => {
                        let nested_name = format!(
                            "{}{}",
                            name.to_case(Case::Pascal),
                            field_name.to_case(Case::Pascal)
                        );
                        let nested_struct = generate_struct(field_type, &nested_name);
                        additional_types.extend(nested_struct.clone());
                        let nested_type = format_ident!("{}", nested_name);
                        quote! { #nested_type }
                    }
                };

                field_tokens.extend(quote! {
                    pub #field_ident: #field_type_tokens,
                });
            }

            quote! {
                pub struct #struct_name {
                    #field_tokens
                }

                #additional_types
            }
        }
        _ => TokenStream::new(),
    }
}

pub fn generate_types(table_name: &str, query_type: &QueryType, is_value: bool) -> TokenStream {
    let mut types = Vec::new();

    match query_type {
        QueryType::Object(fields) => {
            let struct_name = format_ident!("{}Result", table_name.to_case(Case::Pascal));

            if is_value {
                // If it's a VALUE query, return Vec<T>
                let field_types: Vec<_> = fields.values().collect();
                if field_types.len() == 1 {
                    // Single field VALUE query
                    let field_type = generate_field_type(table_name, "", field_types[0], true);
                    types.push(quote! {
                        pub type #struct_name = #field_type;
                    });
                } else {
                    // Multiple fields VALUE query
                    let struct_fields = fields.iter().map(|(name, field_type)| {
                        let field_name = format_ident!("{}", name);
                        let field_type = generate_field_type(table_name, name, field_type, false);
                        quote! { pub #field_name: #field_type }
                    });
                    types.push(quote! {
                        pub struct #struct_name {
                            #(#struct_fields),*
                        }
                    });
                    types.push(quote! {
                        pub type #struct_name = Vec<#struct_name>;
                    });
                }
            } else {
                // Regular SELECT query
                let struct_fields = fields.iter().map(|(name, field_type)| {
                    let field_name = format_ident!("{}", name);
                    let field_type = generate_field_type(table_name, name, field_type, false);
                    quote! { pub #field_name: #field_type }
                });
                types.push(quote! {
                    pub struct #struct_name {
                        #(#struct_fields),*
                    }
                });
            }

            // Generate nested types
            for (name, field_type) in fields {
                if let QueryType::Object(nested_fields) = field_type {
                    let nested_type = generate_types(&format!("{}_{}", table_name, name), field_type, false);
                    types.push(nested_type);
                }
            }
        }
        _ => panic!("Expected object type for table"),
    }

    quote! {
        #(#types)*
    }
}

fn generate_field_type(table_name: &str, field_name: &str, field_type: &QueryType, is_value: bool) -> TokenStream {
    let inner_type = match field_type {
        QueryType::Value(kind) => kind_to_type(kind),
        QueryType::Array(inner) => {
            let inner_type = generate_field_type(table_name, field_name, inner, false);
            quote! { Vec<#inner_type> }
        }
        QueryType::Object(_) => {
            let nested_name = format_ident!("{}_{}",
                table_name.to_case(Case::Pascal),
                field_name.to_case(Case::Pascal)
            );
            quote! { #nested_name }
        }
    };

    if is_value {
        quote! { Vec<#inner_type> }
    } else {
        inner_type
    }
}

fn kind_to_type(kind: &Kind) -> TokenStream {
    match kind {
        Kind::Int => quote! { i64 },
        Kind::Float => quote! { f64 },
        Kind::Decimal => quote! { rust_decimal::Decimal },
        Kind::Bool => quote! { bool },
        Kind::String => quote! { String },
        Kind::Datetime => quote! { chrono::DateTime<chrono::Utc> },
        Kind::Duration => quote! { std::time::Duration },
        Kind::Uuid => quote! { uuid::Uuid },
        Kind::Bytes => quote! { Vec<u8> },
        Kind::Geometry(_) => quote! { geojson::Geometry },
        Kind::Option(inner) => {
            let inner_type = kind_to_type(inner);
            quote! { Option<#inner_type> }
        }
        Kind::Any => quote! { serde_json::Value },
        _ => quote! { serde_json::Value },
    }
}
