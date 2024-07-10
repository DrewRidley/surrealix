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
use syn::parse::ParseStream;
use syn::{parse::Parse, parse_macro_input};
use thiserror::Error;
use tokio::runtime::Runtime;
use syn::spanned::Spanned;
mod types;

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

    #[cfg(debug_assertions)]
    {
        // In debug mode, try to fetch from DB first, then fallback to schema file
        if let Ok(db_url) = env::var("SURREALIX_DB_URL") {
            let runtime = Runtime::new().map_err(SchemaError::RuntimeCreationError)?;
            return runtime.block_on(async {
                let ns = env::var("SURREALIX_NAMESPACE")
                    .map_err(|_| SchemaError::EnvVarNotSet("SURREALIX_NAMESPACE".to_string()))?;
                let db_name = env::var("SURREALIX_DB")
                    .map_err(|_| SchemaError::EnvVarNotSet("SURREALIX_DB".to_string()))?;
                let username = env::var("SURREALIX_USER")
                    .map_err(|_| SchemaError::EnvVarNotSet("SURREALIX_USER".to_string()))?;
                let password = env::var("SURREALIX_PASS")
                    .map_err(|_| SchemaError::EnvVarNotSet("SURREALIX_PASS".to_string()))?;

                // Connect to the database
                let db = Surreal::new::<Http>(db_url)
                    .await
                    .map_err(SchemaError::DatabaseConnectionError)?;
                db.use_ns(&ns)
                    .use_db(&db_name)
                    .await
                    .map_err(SchemaError::DatabaseConnectionError)?;

                // Sign in
                db.signin(Root {
                    username: &username,
                    password: &password,
                })
                .await
                .map_err(SchemaError::DatabaseConnectionError)?;

                let mut result = db
                    .query("INFO FOR DB")
                    .await
                    .map_err(SchemaError::DatabaseConnectionError)?;
                let schema = result.take(0).map_err(|_| SchemaError::SchemaParseError)?;
                Ok(parse_db_schema(schema))
            });
        }
    }

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
    content: proc_macro2::TokenStream,
    span: proc_macro::Span,
}

impl Parse for QueryItem {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let content: proc_macro2::TokenStream = input.parse()?;
        let span = content.span().unwrap();
        Ok(QueryItem { content, span })
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
    let input = parse_macro_input!(input as QueryInput);
    let schema = match fetch_schema() {
        Ok(schema) => {
            println!("Fetched schema:\n{}", schema);
            schema
        }
        Err(e) => {
            let error_message = e.to_string();
            return TokenStream::from(quote! {
                compile_error!(#error_message);
            });
        }
    };

    let query_strings: Vec<String> = input
        .queries
        .iter()
        .map(|item| {
            let query = item.content.to_string();
            query.trim_matches(|c: char| c.is_whitespace() || c == ';').to_string()
        })
        .collect();

    let spans: Vec<proc_macro::Span> = input.queries.iter().map(|item| item.span).collect();

    // Extract template parameters
    let template_params: Vec<String> = input
        .queries
        .iter()
        .flat_map(|item| {
            item.content
                .to_string()
                .split_whitespace()
                .filter(|&word| word.starts_with('{') && word.ends_with('}'))
                .map(|word| word[1..word.len() - 1].to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    let validation_result = surrealix_core::validate_queries(&schema, &query_strings);

    match validation_result {
        Err(errors) => {
            for error in errors {
                let span = spans[error.idx];
                proc_macro::Diagnostic::spanned(span, proc_macro::Level::Error, &error.message)
                    .emit();
            }
            // Return an empty token stream if there are errors
            return TokenStream::new();
        }
        Ok(generated_types) => {
            let template_param_idents: Vec<TokenStream2> = template_params
                .iter()
                .map(|param| {
                    let ident = syn::Ident::new(param, proc_macro2::Span::call_site());
                    quote! { #ident }
                })
                .collect();

            quote! {
                {
                    #(#generated_types)*

                    // // Generate a function that takes the template parameters
                    // fn execute_query(#(#template_param_idents: impl serde::Serialize,)*) -> impl std::future::Future<Output = Result<(), surrealdb::Error>> {
                    //     async move {
                    //         // Here you would use the template_param_idents to construct your query
                    //         // For now, this is just a placeholder
                    //         Ok(())
                    //     }
                    // }
                }
            }
            .into()
        }
    }
}
