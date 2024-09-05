use std::mem::uninitialized;

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod build_query;
mod common;
mod query;

#[proc_macro]
pub fn build_query(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as build_query::parser::BuildQueryInput);

    let schema = match common::schema_loader::load_schema() {
        Ok(schema) => schema,
        Err(e) => {
            return syn::Error::new(proc_macro2::Span::call_site(), e.to_string())
                .to_compile_error()
                .into()
        }
    };

    let Ok(parsed_schema) = surrealdb::sql::parse(&schema) else {
        //We know its an error so this unwrap is okay.
        let error = surrealdb::sql::parse(&schema).err().unwrap();

        return syn::Error::new(proc_macro2::Span::call_site(), error.to_string())
            .to_compile_error()
            .into();
    };

    build_query::generator::generate_code(input, parsed_schema).unwrap()
}
