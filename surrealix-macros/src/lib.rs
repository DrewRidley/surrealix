#![feature(proc_macro_span)]
#![feature(proc_macro_diagnostic)]
use proc_macro::Ident;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::TokenTree;
use quote::format_ident;
use quote::quote;
use std::env;
use std::fs;
use std::path::PathBuf;
use surrealdb::engine::remote::http::Http;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use surrealix_core::analyzer::analyze;
use surrealix_core::code_generator::generate_code;
use surrealix_core::code_generator::generate_single_type_alias;
use surrealix_core::schema::parse_schema;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
use syn::LitStr;
use syn::Token;
use syn::{parse::Parse, parse_macro_input};
use thiserror::Error;
use tokio::runtime::Runtime;

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

fn parse_db_schema(res: surrealdb::sql::Value) -> String {
    res.to_string()
}

fn load_env() -> Result<(), SchemaError> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| SchemaError::EnvVarNotSet("CARGO_MANIFEST_DIR".to_string()))?;
    let mut env_path = PathBuf::from(manifest_dir);
    env_path.push(".env");

    dotenv::from_path(env_path)?;
    Ok(())
}

fn fetch_schema() -> Result<String, SchemaError> {
    load_env()?;

    // Fallback to schema file in debug mode, or primary method in release mode
    let path = env::var("SURREALIX_SCHEMA_PATH")
        .map_err(|_| SchemaError::EnvVarNotSet("SURREALIX_SCHEMA_PATH".to_string()))?;

    let path = if path.starts_with("./") || !path.starts_with('/') {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR")
            .map_err(|_| SchemaError::EnvVarNotSet("CARGO_MANIFEST_DIR".to_string()))?;
        let mut path_buf = PathBuf::from(manifest_dir);
        path_buf.push(path.trim_start_matches("./"));
        path_buf
    } else {
        PathBuf::from(path)
    };

    fs::read_to_string(path).map_err(SchemaError::FileReadError)
}

struct QueryItem {
    content: String,
    span: proc_macro::Span,
}

impl Parse for QueryItem {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let content: syn::LitStr = input.parse()?;
        let span = content.span();
        Ok(QueryItem {
            content: content.value(),
            span: span.unwrap(),
        })
    }
}

struct QueryInput {
    queries: Vec<QueryItem>,
}

impl Parse for QueryInput {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let mut queries = Vec::new();

        while !input.is_empty() {
            let item: QueryItem = input.parse()?;
            queries.push(item);
        }

        Ok(QueryInput { queries })
    }
}

struct QueryTypeInput {
    type_name: proc_macro2::Ident,
    query: LitStr,
}

impl Parse for QueryTypeInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let type_name = input.parse()?;
        input.parse::<Token![,]>()?;
        let query = input.parse()?;
        Ok(QueryTypeInput { type_name, query })
    }
}

#[proc_macro]
pub fn queryType(input: TokenStream) -> TokenStream {
    let QueryTypeInput { type_name, query } = parse_macro_input!(input as QueryTypeInput);
    let query_str = query.value();

    let schema = match fetch_schema() {
        Ok(schema) => schema,
        Err(e) => {
            let error_message = e.to_string();
            return TokenStream::from(quote! {
                compile_error!(#error_message);
            });
        }
    };

    let tables = parse_schema(&schema).unwrap();
    let analysis_result = analyze(tables, query_str);

    if let Some(typed_query) = analysis_result.first() {
        generate_single_type_alias(typed_query, &type_name.to_string()).into()
    } else {
        TokenStream::from(quote! {
            compile_error!("Failed to analyze the query");
        })
    }
}

#[proc_macro]
pub fn query(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as QueryItem);
    let query = input.content;

    let schema = match fetch_schema() {
        Ok(schema) => schema,
        Err(e) => {
            let error_message = e.to_string();
            return TokenStream::from(quote! {
                compile_error!(#error_message);
            });
        }
    };

    // Parse schema into definitions.
    let tables = parse_schema(&schema).unwrap();

    let res = analyze(tables, query.clone());
    let generated_code = generate_code(res);
    let dummy_instance = quote! {
        let dummy: FinalQueryResult = unsafe { std::mem::zeroed() };
        dummy
    };

    quote! {
        {
            #generated_code
            #dummy_instance
        }
    }
    .into()
}
