use crate::types::{QueryType, TypedQuery};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashMap;
use surrealdb::sql::Kind;

pub fn generate_single_type_alias(query: &TypedQuery, type_name: &str) -> TokenStream {
    let type_ident = format_ident!("{}", type_name);
    let (type_def, nested_types) = generate_type_definition(&query.query_type, type_name);

    quote! {
        #nested_types
        pub type #type_ident = #type_def;
    }
}

fn generate_type_definition(query_type: &QueryType, name: &str) -> (TokenStream, TokenStream) {
    match query_type {
        QueryType::Scalar(kind) => (scalar_to_rust_type(kind), TokenStream::new()),
        QueryType::Object(fields) => generate_struct_type(fields, name),
        QueryType::Array(inner, _) => {
            if let Some(inner_type) = inner {
                let (inner_type, nested) =
                    generate_type_definition(&inner_type.query_type, &format!("{}Item", name));
                (quote! { Vec<#inner_type> }, nested)
            } else {
                (quote! { Vec<serde_json::Value> }, TokenStream::new())
            }
        }
        QueryType::Record(table_name) => (
            quote! { surrealix::types::Record<#table_name> },
            TokenStream::new(),
        ),
        QueryType::Option(inner) => {
            let (inner_type, nested) = generate_type_definition(&inner.query_type, name);
            (quote! { Option<#inner_type> }, nested)
        }
    }
}

fn generate_struct_type(
    fields: &HashMap<String, TypedQuery>,
    name: &str,
) -> (TokenStream, TokenStream) {
    let struct_name = format_ident!("{}", to_valid_rust_identifier(name));
    let mut field_defs = TokenStream::new();
    let mut nested_types = TokenStream::new();

    for (field_name, field_type) in fields {
        let field_ident = format_ident!("{}", sanitize_field_name(field_name));
        let nested_name = to_valid_rust_identifier(&format!("{}_{}", name, field_name));
        let (field_type_def, nested) =
            generate_type_definition(&field_type.query_type, &nested_name);
        field_defs.extend(quote! { #field_ident: #field_type_def, });
        nested_types.extend(nested);
    }

    let derive_macro = get_derive_macro();

    let struct_def = quote! {
        #derive_macro
        struct #struct_name {
            #field_defs
        }
    };

    (
        quote! { #struct_name },
        quote! { #nested_types #struct_def },
    )
}

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
            }
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

fn generate_type_and_structs(
    query_type: &QueryType,
    name: &str,
) -> Result<(TokenStream, TokenStream), String> {
    match query_type {
        QueryType::Scalar(kind) => {
            let rust_type = scalar_to_rust_type(kind);
            Ok((quote! { #rust_type }, TokenStream::new()))
        }
        QueryType::Object(fields) => {
            let struct_name = to_valid_rust_identifier(name);
            let struct_ident = format_ident!("{}", struct_name);
            let (field_types, nested_structs) = generate_struct_fields(fields)?;

            let derive_macro = get_derive_macro();

            let struct_def = quote! {
                #derive_macro
                struct #struct_ident {
                    #field_types
                }
            };

            let mut output = nested_structs;
            output.extend(struct_def);
            Ok((quote! { #struct_ident }, output))
        }
        QueryType::Array(inner, _) => {
            if let Some(inner_type) = inner {
                let (inner_rust_type, nested_structs) =
                    generate_type_and_structs(&inner_type.query_type, &format!("{}Item", name))?;
                Ok((quote! { Vec<#inner_rust_type> }, nested_structs))
            } else {
                Ok((quote! { Vec<serde_json::Value> }, TokenStream::new()))
            }
        }
        QueryType::Record(table_name) => {
            Ok((quote! { surreal::Record<#table_name> }, TokenStream::new()))
        }
        QueryType::Option(inner) => {
            let (inner_rust_type, nested_structs) =
                generate_type_and_structs(&inner.query_type, name)?;
            Ok((quote! { Option<#inner_rust_type> }, nested_structs))
        }
    }
}

fn get_derive_macro() -> TokenStream {
    #[cfg(feature = "serde")]
    {
        quote! { #[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize)] }
    }
    #[cfg(feature = "miniserde")]
    {
        quote! { #[derive(Debug, Clone, ::miniserde::Serialize, ::miniserde::Deserialize)] }
    }
    #[cfg(not(any(feature = "serde", feature = "miniserde")))]
    {
        quote! { #[derive(Debug, Clone)] }
    }
}

fn generate_struct_fields(
    fields: &HashMap<String, TypedQuery>,
) -> Result<(TokenStream, TokenStream), String> {
    let mut field_types = TokenStream::new();
    let mut nested_structs = TokenStream::new();

    for (field_name, field_type) in fields {
        let sanitized_field_name = sanitize_field_name(field_name);
        let field_ident = format_ident!("{}", sanitized_field_name);
        let type_name = to_valid_rust_identifier(&format!("{}Type", field_name));
        let type_ident = format_ident!("{}", type_name);
        let (rust_type, more_structs) =
            generate_type_and_structs(&field_type.query_type, &type_name)?;

        #[cfg(any(feature = "serde", feature = "miniserde"))]
        {
            field_types.extend(quote! {
                #[serde(rename = #field_name)]
                #field_ident: #rust_type,
            });
        }
        #[cfg(not(any(feature = "serde", feature = "miniserde")))]
        {
            field_types.extend(quote! {
                #field_ident: #rust_type,
            });
        }

        nested_structs.extend(more_structs);
    }

    Ok((field_types, nested_structs))
}

fn sanitize_field_name(name: &str) -> String {
    let sanitized = name
        .replace("::", "_")
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
    if result
        .chars()
        .next()
        .map_or(true, |c| !c.is_alphabetic() && c != '_')
    {
        result.insert(0, '_');
    }

    // If the result is a Rust keyword, append an underscore
    if is_rust_keyword(&result) {
        result.push('_');
    }

    result
}

fn is_rust_keyword(s: &str) -> bool {
    matches!(
        s,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    )
}

fn scalar_to_rust_type(kind: &Kind) -> TokenStream {
    match kind {
        Kind::String => quote! { String },
        Kind::Int => quote! { i64 },
        Kind::Float => quote! { f64 },
        Kind::Bool => quote! { bool },
        Kind::Datetime => quote! { surrealix::types::DateTime },
        Kind::Duration => quote! { surrealix::types::Duration },
        Kind::Uuid => quote! { [u8; 16] },
        Kind::Record(_) => quote! { surrealix::types::RecordLink },
        _ => {
            #[cfg(feature = "serde")]
            {
                quote! { serde_json::Value }
            }
            #[cfg(feature = "miniserde")]
            {
                quote! { miniserde::json::Value }
            }
            #[cfg(not(any(feature = "serde", feature = "miniserde")))]
            {
                quote! { () }
            }
        }
    }
}
