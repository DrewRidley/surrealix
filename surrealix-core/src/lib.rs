use convert_case::{Case, Casing};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::{collections::HashMap, num::NonZeroUsize};
use surrealdb::sql::{
    parse,
    statements::{DefineFieldStatement, DefineStatement, DefineTableStatement, SelectStatement},
    Kind, Part, Permissions, Statement, Strand, Value, View,
};

mod parser;

#[derive(Debug)]
pub struct ValidationError {
    pub message: String,
    pub idx: usize,
}

#[derive(Debug)]
pub enum FieldType {
    Inline(Kind),
    Array((Box<FieldDefinition>, Option<NonZeroUsize>)),
    Object(Vec<FieldDefinition>),
}

#[derive(Debug)]
pub struct FieldDefinition {
    pub name: String,
    pub field_type: FieldType,
    pub permissions: Permissions,
    pub value: Option<Value>,
}

#[derive(Debug)]
pub struct TableDefinition {
    pub name: String,
    pub permissions: Permissions,
    pub fields: Vec<FieldDefinition>,
}

pub fn validate_queries(
    schema: &str,
    queries: &[String],
) -> Result<Vec<TokenStream>, Vec<ValidationError>> {
    let definitions = generate_definitions(schema).unwrap();
    let mut generated_types = vec![];

    let mut errors = Vec::new();

    for (idx, query) in queries.iter().enumerate() {
        match parse(query) {
            Ok(ast) => {
                for stmt in ast {
                    if let Statement::Select(select_stmt) = stmt {
                        generated_types
                            .push(parser::build_select_response(&definitions, &select_stmt));
                    }
                }
            }
            Err(e) => {
                errors.push(ValidationError {
                    message: e.to_string(),
                    idx,
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(generated_types)
    } else {
        Err(errors)
    }
}

fn create_table_definition(table_stmt: &DefineTableStatement) -> TableDefinition {
    TableDefinition {
        name: table_stmt.name.to_string(),
        permissions: table_stmt.permissions.clone(),
        fields: vec![],
    }
}

fn generate_definitions(schema: &str) -> Result<Vec<TableDefinition>, String> {
    let schema = parse(schema).map_err(|e| format!("Failed to parse schema: {}", e))?;

    println!("Parsed schema: {:?}", schema);

    let mut table_definitions: Vec<_> = schema
        .iter()
        .filter_map(|stmt| {
            if let Statement::Define(DefineStatement::Table(tbl_def)) = stmt {
                println!("Found table definition: {:?}", tbl_def);
                Some(create_table_definition(tbl_def))
            } else {
                None
            }
        })
        .collect();

    println!(
        "Table definitions after initial pass: {:?}",
        table_definitions
    );

    for stmt in schema.iter() {
        if let Statement::Define(DefineStatement::Field(field_def)) = stmt {
            println!("Processing field definition: {:?}", field_def);
            let table_name = field_def.what.to_string();
            println!("Looking for table: {}", table_name);
            let table = table_definitions
                .iter_mut()
                .find(|t| t.name.to_lowercase() == table_name.to_lowercase());

            match table {
                Some(table) => {
                    println!("Found table: {}", table_name);
                    let field_parts: Vec<_> = field_def.name.iter().collect();
                    add_or_update_field(&mut table.fields, &field_parts, field_def);
                }
                None => {
                    println!("Failed to find table: {}", table_name);
                    return Err(format!(
                        "Failed to find table corresponding to field: {}",
                        table_name
                    ));
                }
            }
        }
    }

    println!("Final table definitions: {:?}", table_definitions);

    Ok(table_definitions)
}

fn add_or_update_field(
    fields: &mut Vec<FieldDefinition>,
    field_parts: &[&Part],
    field_def: &DefineFieldStatement,
) {
    if field_parts.is_empty() {
        return;
    }

    let (current_part, remaining_parts) = field_parts.split_first().unwrap();
    let is_last = remaining_parts.is_empty();

    match current_part {
        Part::Field(field_name) => {
            let field_name = field_name.to_string();
            let field_index = fields.iter().position(|f| f.name == field_name);

            if is_last {
                if let Some(index) = field_index {
                    // Update existing field
                    let field = &mut fields[index];
                    field.field_type = create_field_type(field_def);
                    field.permissions = field_def.permissions.clone();
                    field.value = field_def.value.clone();
                } else {
                    // Add new field
                    fields.push(FieldDefinition {
                        name: field_name,
                        field_type: create_field_type(field_def),
                        permissions: field_def.permissions.clone(),
                        value: field_def.value.clone(),
                    });
                }
            } else {
                if let Some(index) = field_index {
                    // Navigate to nested field
                    match &mut fields[index].field_type {
                        FieldType::Object(nested_fields) => {
                            add_or_update_field(nested_fields, remaining_parts, field_def);
                        }
                        FieldType::Array((box_def, _)) => {
                            if remaining_parts.len() == 1 && remaining_parts[0] == &Part::All {
                                // Update array element type
                                *box_def = Box::new(FieldDefinition {
                                    name: "*".to_string(),
                                    field_type: create_field_type(field_def),
                                    permissions: field_def.permissions.clone(),
                                    value: field_def.value.clone(),
                                });
                            } else if let FieldType::Object(nested_fields) = &mut box_def.field_type
                            {
                                add_or_update_field(nested_fields, remaining_parts, field_def);
                            }
                        }
                        _ => panic!("Expected object or array for nested field"),
                    }
                } else {
                    // Create new nested field
                    let mut new_field = FieldDefinition {
                        name: field_name,
                        field_type: FieldType::Object(vec![]),
                        permissions: Permissions::default(),
                        value: None,
                    };
                    if let FieldType::Object(nested_fields) = &mut new_field.field_type {
                        add_or_update_field(nested_fields, remaining_parts, field_def);
                    }
                    fields.push(new_field);
                }
            }
        }
        Part::All => {
            if is_last {
                // Update or add the "*" field for arrays
                let field_index = fields.iter().position(|f| f.name == "*");
                if let Some(index) = field_index {
                    let field = &mut fields[index];
                    field.field_type = create_field_type(field_def);
                    field.permissions = field_def.permissions.clone();
                    field.value = field_def.value.clone();
                } else {
                    fields.push(FieldDefinition {
                        name: "*".to_string(),
                        field_type: create_field_type(field_def),
                        permissions: field_def.permissions.clone(),
                        value: field_def.value.clone(),
                    });
                }
            } else {
                panic!("All selector (*) should only be used at the end of a field path");
            }
        }
        _ => panic!("Unexpected part type: {:?}", current_part),
    }
}

fn create_field_type(field_def: &DefineFieldStatement) -> FieldType {
    match field_def.kind.as_ref().unwrap() {
        Kind::Object => FieldType::Object(vec![]),
        Kind::Record(_) => todo!("Record impl"),
        Kind::Option(inner_kind) => FieldType::Inline(Kind::Option(inner_kind.clone())),
        Kind::Either(_) => todo!("Tuple impl"),
        Kind::Set(_, _) => todo!("Set impl!"),
        Kind::Array(kind, len) => FieldType::Array((
            Box::new(FieldDefinition {
                name: "*".to_string(),
                field_type: FieldType::Inline((**kind).clone()),
                permissions: field_def.permissions.clone(),
                value: field_def.value.clone(),
            }),
            None,
        )),
        kind => FieldType::Inline(kind.clone()),
    }
}

pub fn generate_rust_types(table_definitions: &[TableDefinition]) -> TokenStream {
    let mut types = Vec::new();

    for table in table_definitions {
        types.extend(generate_table_types(table));
    }

    quote! {
        #(#types)*
    }
}

fn generate_table_types(table: &TableDefinition) -> Vec<TokenStream> {
    let mut types = Vec::new();
    let main_struct = generate_struct_for_table(table);
    types.push(main_struct);

    // Generate types for nested objects
    for field in &table.fields {
        if let FieldType::Object(nested_fields) = &field.field_type {
            let nested_struct = generate_nested_struct(&table.name, &field.name, nested_fields);
            types.push(nested_struct);
        }
    }

    types
}

fn generate_struct_for_table(table: &TableDefinition) -> TokenStream {
    let struct_name = format_ident!("{}", table.name.to_case(Case::Pascal));
    let fields = table
        .fields
        .iter()
        .map(|field| generate_field(field, &table.name.to_string()));

    quote! {
        pub struct #struct_name {
            #(#fields),*
        }
    }
}

fn generate_nested_struct(
    table_name: &str,
    field_name: &str,
    nested_fields: &[FieldDefinition],
) -> TokenStream {
    let struct_name = format_ident!(
        "{}{}",
        table_name.to_case(Case::Pascal),
        field_name.to_case(Case::Pascal)
    );
    let fields = nested_fields
        .iter()
        .map(|field| generate_field(field, table_name));

    quote! {
        pub struct #struct_name {
            #(#fields),*
        }
    }
}

fn generate_field(field: &FieldDefinition, name: &str) -> TokenStream {
    let field_name = format_ident!("{}", field.name);
    let field_type = generate_field_type(field, name);

    quote! {
        pub #field_name: #field_type
    }
}

fn generate_field_type(field: &FieldDefinition, table_name: &str) -> TokenStream {
    match &field.field_type {
        FieldType::Inline(kind) => generate_inline_type(kind),
        FieldType::Array((box_def, _)) => {
            let element_type = generate_field_type(box_def, table_name);
            quote! { Vec<#element_type> }
        }
        FieldType::Object(nested_fields) => {
            if nested_fields.is_empty() {
                quote! { serde_json::Value }
            } else {
                let struct_name = format_ident!(
                    "{}{}",
                    table_name.to_case(Case::Pascal),
                    field.name.to_case(Case::Pascal)
                );
                quote! { #struct_name }
            }
        }
    }
}

fn generate_inline_type(kind: &Kind) -> TokenStream {
    match kind {
        Kind::Any => quote! { serde_json::Value },
        Kind::Array(inner_kind, _) => {
            let inner_type = generate_inline_type(inner_kind);
            quote! { Vec<#inner_type> }
        }
        Kind::Bool => quote! { bool },
        Kind::Bytes => quote! { Vec<u8> },
        Kind::Datetime => quote! { chrono::DateTime<chrono::Utc> },
        Kind::Decimal => quote! { rust_decimal::Decimal },
        Kind::Duration => quote! { std::time::Duration },
        Kind::Float => quote! { f64 },
        Kind::Int => quote! { i64 },
        Kind::Number => quote! { f64 },
        Kind::Object => quote! { serde_json::Value },
        Kind::String => quote! { String },
        Kind::Uuid => quote! { uuid::Uuid },
        Kind::Record(_) => quote! { serde_json::Value },
        Kind::Geometry(_) => quote! { serde_json::Value },
        Kind::Option(inner_kind) => {
            let inner_type = generate_inline_type(inner_kind);
            quote! { Option<#inner_type> }
        }
        _ => quote! { serde_json::Value },
    }
}
