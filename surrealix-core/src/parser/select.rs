use super::{QueryType, TableDefinition};
use crate::parser::types::{generate_query_types, generate_types};
use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use std::collections::HashMap;
use surrealdb::sql::{statements::SelectStatement, Field, Kind, Value};

pub fn build_select_response(tables: &[TableDefinition], select: &SelectStatement) -> TokenStream {
    println!("Building select response for: \n{:#?}\n", select);
    println!("Available tables: \n{:#?}\n", tables);

    let table = match select.what.first() {
        Some(Value::Table(name)) => {
            let table_name = name.to_string();
            println!("Looking for table: {}", table_name);
            tables
                .iter()
                .find(|t| t.name.to_lowercase() == table_name.to_lowercase())
                .unwrap_or_else(|| {
                    println!("Table not found: {}", table_name);
                    panic!("Table not found: {}", table_name)
                })
        }
        _ => {
            println!("Invalid SELECT statement: {:?}", select);
            return TokenStream::new();
        }
    };

    println!("Found table: {:?}", table);

    let table_type = QueryType::from_table(table);
    let query_type = parse_select_fields(select, &table_type);

    generate_types(&table.name, &query_type, select.expr.1)
}

fn parse_select_fields(select: &SelectStatement, table_type: &QueryType) -> QueryType {
    if let QueryType::Object(table_fields) = table_type {
        let mut query_fields = HashMap::new();

        for expr in select.expr.iter() {
            match expr {
                Field::All => return table_type.clone(),
                Field::Single { expr, alias } => {
                    let field_name = alias
                        .as_ref()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| expr.to_string());
                    let parts: Vec<&str> = field_name.split('.').collect();

                    let mut current_fields = table_fields;
                    let mut current_type = None;

                    for (i, part) in parts.iter().enumerate() {
                        if let Some(field_type) = current_fields.get(*part) {
                            if i == parts.len() - 1 {
                                current_type = Some(field_type.clone());
                            } else if let QueryType::Object(nested_fields) = field_type {
                                current_fields = nested_fields;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    if let Some(field_type) = current_type {
                        if parts.len() > 1 {
                            let mut nested_type = field_type;
                            for part in parts.iter().rev().skip(1) {
                                nested_type = QueryType::Object(HashMap::from([(part.to_string(), nested_type)]));
                            }
                            query_fields.insert(parts[0].to_string(), nested_type);
                        } else {
                            query_fields.insert(field_name, field_type);
                        }
                    }
                }
            }
        }

        QueryType::Object(query_fields)
    } else {
        table_type.clone()
    }
}
