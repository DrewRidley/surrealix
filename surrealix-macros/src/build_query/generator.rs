use std::collections::HashMap;

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use surrealdb::sql::Query;
use surrealix_core::{
    analyzer::analyze,
    ast::{ObjectType, ScalarType, TypeAST},
    errors,
};
use thiserror::Error;

use super::parser::BuildQueryInput;

#[derive(Debug, Error)]
pub enum QueryBuilderError {
    #[error("The specified SurrealQL is invalid: {0}")]
    ParseError(#[from] surrealdb::error::Db),
    #[error("Failed to analyze the query")]
    AnalysisError(#[from] errors::AnalysisError),
}

pub fn generate_code(
    input: BuildQueryInput,
    schema: Query,
) -> Result<TokenStream, QueryBuilderError> {
    let query_str = input.query.value();
    let parsed_query = surrealdb::sql::parse(&query_str)?;

    let analyzed = analyze(schema, parsed_query)?;

    let mut type_definitions = Vec::new();
    let mut type_aliases = Vec::new();
    let mut generated_types = HashMap::new();

    for (index, ast) in analyzed.iter().enumerate() {
        let (type_name, type_def) = generate_type_definition(ast, &mut generated_types);
        type_definitions.extend(type_def);

        let alias_name = if analyzed.len() == 1 {
            format_ident!("QueryResult")
        } else {
            format_ident!("QueryResult{}", index + 1)
        };

        let alias = quote! {
            pub type #alias_name = #type_name;
        };
        type_aliases.push(alias);
    }

    let module_name = format_ident!("adult_users");
    let alias_name = format_ident!("AdultUsers");

    let generated_code = quote! {
        pub struct #alias_name;

        impl #alias_name {
            pub fn execute() -> Result<QueryResult, surrealix::Error> {
                // Implementation of execute method
                todo!("Implement execute method")
            }
        }

        pub mod #module_name {
            use super::*;

            #(#type_definitions)*

            #(#type_aliases)*
        }
    };

    Ok(generated_code.into())
}

fn generate_type_definition(
    ast: &TypeAST,
    generated_types: &mut HashMap<String, TokenStream2>,
) -> (TokenStream2, Vec<TokenStream2>) {
    match ast {
        TypeAST::Object(obj) => generate_object_definition(obj, generated_types),
        TypeAST::Array(inner) => {
            let (inner_type, inner_defs) = generate_type_definition(&inner.0, generated_types);
            (quote! { Vec<#inner_type> }, inner_defs)
        }
        TypeAST::Option(inner) => {
            let (inner_type, inner_defs) = generate_type_definition(inner, generated_types);
            (quote! { Option<#inner_type> }, inner_defs)
        }
        TypeAST::Scalar(scalar) => (scalar_type_to_rust_type(scalar), vec![]),
        TypeAST::Record(table) => {
            let type_name = format_ident!("{}", table.to_case(Case::Pascal));
            (quote! { RecordLink<#type_name> }, vec![])
        }
        TypeAST::Union(_) => (quote! { serde_json::Value }, vec![]),
    }
}

fn generate_object_definition(
    obj: &ObjectType,
    generated_types: &mut HashMap<String, TokenStream2>,
) -> (TokenStream2, Vec<TokenStream2>) {
    let mut type_definitions = Vec::new();
    let type_name = generate_object_name(obj);

    if let Some(existing_def) = generated_types.get(&type_name.to_string()) {
        return (existing_def.clone(), type_definitions);
    }

    let fields = obj.fields.iter().map(|(name, field_info)| {
        let field_name = format_ident!("{}", name);
        let (field_type, mut field_defs) =
            generate_type_definition(&field_info.ast, generated_types);
        type_definitions.append(&mut field_defs);
        quote! { pub #field_name: #field_type }
    });

    let type_def = quote! {
        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        pub struct #type_name {
            #(#fields,)*
        }
    };

    type_definitions.push(type_def.clone());
    generated_types.insert(type_name.to_string(), quote! { #type_name });

    (quote! { #type_name }, type_definitions)
}

fn generate_object_name(obj: &ObjectType) -> Ident {
    let path = obj
        .fields
        .values()
        .next()
        .map(|field| field.meta.original_path.clone())
        .unwrap_or_else(|| vec!["Unknown".to_string()]);

    let name = if path.len() > 1 {
        if path[0] == path[1] {
            // This is the root object, just use the table name
            path[0].clone()
        } else {
            // For nested objects, use all segments except the last one
            path[..path.len() - 1].join("_")
        }
    } else {
        "Unknown".to_string()
    };

    format_ident!("{}", name.to_case(Case::Pascal))
}

fn scalar_type_to_rust_type(scalar_type: &ScalarType) -> TokenStream2 {
    match scalar_type {
        ScalarType::String => quote! { String },
        ScalarType::Integer => quote! { i64 },
        ScalarType::Number => quote! { f64 },
        ScalarType::Float => quote! { f32 },
        ScalarType::Boolean => quote! { bool },
        ScalarType::Point => quote! { Point },
        ScalarType::Geometry => quote! { Geometry },
        ScalarType::Set => quote! { std::collections::HashSet<String> },
        ScalarType::Datetime => quote! { chrono::DateTime<chrono::Utc> },
        ScalarType::Duration => quote! { std::time::Duration },
        ScalarType::Bytes => quote! { Vec<u8> },
        ScalarType::Uuid => quote! { uuid::Uuid },
        ScalarType::Any => quote! { serde_json::Value },
        ScalarType::Null => quote! { () },
    }
}
