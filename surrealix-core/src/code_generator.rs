use proc_macro2::{TokenStream, Ident};
use quote::{quote, format_ident};
use std::collections::HashMap;
use crate::types::{TypedQuery, QueryType};
use surrealdb::sql::Kind;

pub fn generate_code(queries: Vec<TypedQuery>) -> TokenStream {
    let mut output = TokenStream::new();
    let mut result_types = Vec::new();

    for (index, query) in queries.iter().enumerate() {
        let query_name = format_ident!("Query{}Result", index + 1);
        match generate_type_and_structs(&query.query_type, &query_name.to_string()) {
            Ok((result_type, structs)) => {
                output.extend(structs);
                output.extend(quote! {
                    type #query_name = #result_type;
                });
                result_types.push(query_name);
            },
            Err(e) => {
                eprintln!("Error generating code for query {}: {}", index + 1, e);
                return quote! { compile_error!(#e); }.into();
            }
        }
    }

    let final_result_type = if result_types.len() > 1 {
        quote! { (#(#result_types),*) }
    } else {
        quote! { #(#result_types)* }
    };

    output.extend(quote! {
        type FinalQueryResult = #final_result_type;
    });

    output
}




fn generate_type_and_structs(query_type: &QueryType, name: &str) -> Result<(TokenStream, TokenStream), String> {
    match query_type {
        QueryType::Scalar(kind) => {
            let rust_type = scalar_to_rust_type(kind);
            Ok((quote! { #rust_type }, TokenStream::new()))
        },
        QueryType::Object(fields) => {
            let struct_name = to_valid_rust_identifier(name);
            let struct_ident = format_ident!("{}", struct_name);
            let (field_types, nested_structs) = generate_struct_fields(fields)?;


            //#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            let struct_def = quote! {
                struct #struct_ident {
                    #field_types
                }
            };

            let mut output = nested_structs;
            output.extend(struct_def);
            Ok((quote! { #struct_ident }, output))
        },
        QueryType::Array(inner, _) => {
            if let Some(inner_type) = inner {
                let (inner_rust_type, nested_structs) = generate_type_and_structs(&inner_type.query_type, &format!("{}Item", name))?;
                Ok((quote! { Vec<#inner_rust_type> }, nested_structs))
            } else {
                Ok((quote! { Vec<serde_json::Value> }, TokenStream::new()))
            }
        },
        QueryType::Record(table_name) => {
            Ok((quote! { surreal::Record<#table_name> }, TokenStream::new()))
        },
        QueryType::Option(inner) => {
            let (inner_rust_type, nested_structs) = generate_type_and_structs(&inner.query_type, name)?;
            Ok((quote! { Option<#inner_rust_type> }, nested_structs))
        },
    }
}

fn generate_struct_fields(fields: &HashMap<String, TypedQuery>) -> Result<(TokenStream, TokenStream), String> {
    let mut field_types = TokenStream::new();
    let mut nested_structs = TokenStream::new();

    for (field_name, field_type) in fields {
        let sanitized_field_name = sanitize_field_name(field_name);
        let field_ident = format_ident!("{}", sanitized_field_name);
        let type_name = to_valid_rust_identifier(&format!("{}Type", field_name));
        let type_ident = format_ident!("{}", type_name);
        let (rust_type, more_structs) = generate_type_and_structs(&field_type.query_type, &type_name)?;
        field_types.extend(quote! {
            #[serde(rename = #field_name)]
            #field_ident: #rust_type,
        });
        nested_structs.extend(more_structs);
    }

    Ok((field_types, nested_structs))
}

fn sanitize_field_name(name: &str) -> String {
    let sanitized = name.replace("::", "_")
        .replace(".", "_")
        .replace("-", "_")
        .to_lowercase();

    // Ensure the field name is a valid Rust identifier
    if sanitized.chars().next().unwrap_or('_').is_numeric() {
        format!("_{}", sanitized)
    } else {
        sanitized
    }
}

fn to_valid_rust_identifier(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in s.chars() {
        if c.is_alphanumeric() {
            if capitalize_next {
                result.push(c.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(c);
            }
        } else {
            capitalize_next = true;
        }
    }

    // Ensure the identifier starts with a letter or underscore
    if result.chars().next().map_or(true, |c| !c.is_alphabetic() && c != '_') {
        result.insert(0, '_');
    }

    // If the result is a Rust keyword, append an underscore
    if is_rust_keyword(&result) {
        result.push('_');
    }

    result
}

fn is_rust_keyword(s: &str) -> bool {
    matches!(s, "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern" |
                "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod" |
                "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self" | "static" | "struct" |
                "super" | "trait" | "true" | "type" | "unsafe" | "use" | "where" | "while" | "async" |
                "await" | "dyn" | "abstract" | "become" | "box" | "do" | "final" | "macro" |
                "override" | "priv" | "typeof" | "unsized" | "virtual" | "yield")
}

fn scalar_to_rust_type(kind: &Kind) -> TokenStream {
    match kind {
        Kind::String => quote! { String },
        Kind::Int => quote! { i64 },
        Kind::Float => quote! { f64 },
        Kind::Bool => quote! { bool },
        Kind::Datetime => quote! { chrono::DateTime<chrono::Utc> },
        Kind::Duration => quote! { std::time::Duration },
        Kind::Uuid => quote! { uuid::Uuid },
        _ => quote! { serde_json::Value },
    }
}
