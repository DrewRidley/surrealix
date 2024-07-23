#![feature(proc_macro_span)]
#![feature(proc_macro_diagnostic)]
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::TokenTree;
use quote::quote;
use std::env;
use std::fs;
use std::path::PathBuf;
use surrealdb::engine::remote::http::Http;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use surrealix_core::analyzer::analyze;
use surrealix_core::code_generator::generate_code;
use surrealix_core::schema::parse_schema;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
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

#[proc_macro]
pub fn query(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as QueryItem);
    let query = input.content;

    println!("Query string: {:?}", query);

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

    quote! {
        {
            #generated_code

            ()
        }
    }
    .into()
}
