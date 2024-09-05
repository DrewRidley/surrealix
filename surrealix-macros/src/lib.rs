use convert_case::Case;
use convert_case::Casing;
use proc_macro2::Ident;
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;
use quote::quote;
use std::env;
use std::fs;
use std::path::PathBuf;
use surrealix_core::analyzer::analyze;
use surrealix_core::ast::ObjectType;
use surrealix_core::ast::ScalarType;
use surrealix_core::ast::TypeAST;
use syn::parse::ParseStream;
use syn::LitStr;
use syn::Token;
use syn::{parse::Parse, parse_macro_input, Result as SynResult};
use thiserror::Error;

mod utils;
mod prepared;

#[derive(Error, Debug)]
enum SchemaError {
    #[error("Environment variable not set: {0}")]
    EnvVarNotSet(String),

    #[error("Failed to read schema file: {0}")]
    FileReadError(std::io::Error),

    #[error("Database connection error: {0}")]
    DatabaseConnectionError(#[from] surrealdb::Error),

    #[error("Runtime creation error: {0}")]
    RuntimeCreationError(#[from] tokio::io::Error),

    #[error("Failed to parse database schema")]
    SchemaParseError,

    #[error("Failed to load .env file: {0}")]
    DotEnvError(#[from] dotenv::Error),
}



struct BuildQueryInput {
    name: Ident,
    aliases: Vec<(Ident, String)>,
    query: LitStr,
}

impl Parse for BuildQueryInput {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut aliases = Vec::new();
        while !input.peek(LitStr) {
            let alias: Ident = input.parse()?;
            input.parse::<Token![=>]>()?;
            let mut path = String::new();
            loop {
                let ident: Ident = input.parse()?;
                path.push_str(&ident.to_string());
                if input.peek(Token![.]) {
                    input.parse::<Token![.]>()?;
                    path.push('.');
                } else {
                    break;
                }
            }
            aliases.push((alias, path));
            input.parse::<Token![,]>()?;
        }

        let query: LitStr = input.parse()?;

        Ok(BuildQueryInput {
            name,
            aliases,
            query,
        })
    }
}

#[proc_macro]
pub fn build_query(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let BuildQueryInput {
        name,
        aliases,
        query,
    } = parse_macro_input!(input as BuildQueryInput);

    let struct_name = &name;

    let schema = fetch_schema().unwrap();
    let parsed_schema = surrealdb::sql::parse(&schema).unwrap();
    let parsed_query = surrealdb::sql::parse(&query.value().clone()).unwrap();

    let analyzed = analyze(parsed_schema, parsed_query);

    let (is_array, inner_type) = if let Some(ast) = analyzed.first() {
        match ast {
            TypeAST::Array(inner) => (true, &inner.0),
            _ => (false, ast),
        }
    } else {
        return quote! {
            compile_error!("Failed to analyze the query");
        }
        .into();
    };

    let (return_type, struct_def, additional_types) = match inner_type {
        TypeAST::Object(obj) => {
            let mut additional_types = Vec::new();
            let fields = generate_fields(inner_type, &aliases, "", &mut additional_types);
            let return_type = if is_array {
                quote! { Vec<#struct_name> }
            } else {
                quote! { #struct_name }
            };
            let struct_def = quote! {
                #[derive(Debug, serde::Serialize, serde::Deserialize)]
                pub struct #struct_name {
                    #fields
                }
            };
            (return_type, struct_def, additional_types)
        }
        TypeAST::Scalar(scalar_type) => {
            let rust_type = scalar_type_to_rust_type(scalar_type);
            let return_type = if is_array {
                quote! { Vec<#rust_type> }
            } else {
                quote! { #rust_type }
            };
            let struct_def = quote! {
                #[derive(Debug)]
                pub struct #struct_name;
            };
            (return_type, struct_def, Vec::new())
        }
        _ => {
            return quote! {
                compile_error!("Unsupported query result type");
            }
            .into();
        }
    };

    let execute_impl = quote! {
        pub fn execute() -> Result<#return_type, ()> {
            todo!("Implement execute method")
        }
    };

    let expanded = quote! {
        use surrealix::RecordLink;

        #struct_def

        impl #struct_name {
            #execute_impl
        }

        #(#additional_types)*
    };

    proc_macro::TokenStream::from(expanded)
}

fn generate_object_name(obj: &ObjectType) -> Ident {
    let table_name = obj
        .fields
        .values()
        .filter_map(|field| field.meta.original_path.first())
        .next()
        .unwrap_or(&String::from("UnknownTable"))
        .to_string();

    format_ident!("{}", table_name.to_case(Case::Pascal))
}

fn generate_field_name(field_name: &str) -> Ident {
    format_ident!("{}", field_name.replace(".", "_").to_case(Case::Snake))
}

fn generate_fields(
    ast: &TypeAST,
    aliases: &[(Ident, String)],
    path: &str,
    additional_types: &mut Vec<TokenStream2>,
) -> TokenStream2 {
    match ast {
        TypeAST::Object(obj) => {
            let fields = obj.fields.iter().map(|(name, field_info)| {
                let field_name = generate_field_name(name);
                let field_path = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", path, name)
                };
                let field_type =
                    generate_field_type(&field_info.ast, aliases, &field_path, additional_types);
                quote! { pub #field_name: #field_type }
            });
            quote! { #(#fields,)* }
        }
        _ => quote! {},
    }
}

fn generate_field_type(
    ast: &TypeAST,
    aliases: &[(Ident, String)],
    path: &str,
    additional_types: &mut Vec<TokenStream2>,
) -> TokenStream2 {
    match ast {
        TypeAST::Scalar(scalar_type) => scalar_type_to_rust_type(scalar_type),
        TypeAST::Object(obj) => {
            let type_name = format_ident!("{}", path.replace(".", "_").to_case(Case::Pascal));
            let fields = generate_fields(ast, aliases, path, additional_types);
            let type_def = quote! {
                #[derive(Debug, serde::Serialize, serde::Deserialize)]
                pub struct #type_name {
                    #fields
                }
            };
            additional_types.push(type_def);
            quote! { #type_name }
        }
        TypeAST::Array(inner) => {
            let inner_type = generate_field_type(&inner.0, aliases, path, additional_types);
            quote! { Vec<#inner_type> }
        }
        TypeAST::Option(inner) => {
            let inner_type = generate_field_type(inner, aliases, path, additional_types);
            quote! { Option<#inner_type> }
        }
        TypeAST::Record(_) => {
            quote! { RecordLink }
        }
        TypeAST::Union(_) => quote! { serde_json::Value },
    }
}

fn scalar_type_to_rust_type(scalar_type: &ScalarType) -> TokenStream2 {
    match scalar_type {
        ScalarType::String => quote! { String },
        ScalarType::Integer => quote! { i64 },
        ScalarType::Number => quote! { f64 },
        ScalarType::Float => quote! { f32 },
        ScalarType::Boolean => quote! { bool },
        ScalarType::Point => quote! { Point }, // You might need to define this type
        ScalarType::Geometry => quote! { Geometry }, // You might need to define this type
        ScalarType::Set => quote! { std::collections::HashSet<String> },
        ScalarType::Datetime => quote! { u64 },
        ScalarType::Duration => quote! { std::time::Duration },
        ScalarType::Bytes => quote! { Vec<u8> },
        ScalarType::Uuid => quote! { Uuid },
        ScalarType::Any => quote! { serde_json::Value },
        ScalarType::Null => quote! { () },
    }
}

// #[proc_macro]
// pub fn build_query(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
//     let BuildQueryInput {
//         name,
//         aliases,
//         query,
//     } = parse_macro_input!(input as BuildQueryInput);

//     let alias_tokens = aliases.iter().map(|(alias, path)| {
//         quote! { (stringify!(#alias), #path) }
//     });

//     let expanded = quote! {
//         {
//             println!("Building query: {}", stringify!(#name));
//             let aliases = [#(#alias_tokens),*];
//             for (alias, path) in &aliases {
//                 println!("Alias: {} -> {}", alias, path);
//             }
//             println!("Query: {}", #query);
//             todo!("Implement query building logic")
//         }
//     };

//     proc_macro::TokenStream::from(expanded)
// }
//
//

#[proc_macro]
pub fn query(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as QueryItem);
    let query = input.content;

    // let schema = match fetch_schema() {
    //     Ok(schema) => schema,
    //     Err(e) => {
    //         let error_message = e.to_string();
    //         return TokenStream::from(quote! {
    //             compile_error!(#error_message);
    //         });
    //     }
    // };

    // Parse schema into definitions.
    //let tables = parse_schema(&schema).unwrap();

    // let res = analyze(tables, query.clone());
    // let generated_code = generate_code(res);
    // let dummy_instance = quote! {
    //     let dummy: FinalQueryResult = unsafe { std::mem::zeroed() };
    //     dummy
    // };

    quote! {
        {

        }
    }
    .into()
}
