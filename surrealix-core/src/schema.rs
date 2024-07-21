use std::collections::HashMap;
use std::num::NonZeroU64;
use surrealdb::sql::{statements::DefineStatement, Idiom, Kind, Part, Permissions, Statement};
use thiserror::Error;

use crate::types::{QueryType, TypedQuery};

#[derive(Error, Debug)]
pub enum SchemaParseError {
    #[error("Invalid SurrealQL syntax: {0}")]
    InvalidSyntax(#[from] surrealdb::error::Db),

    #[error("Reference to non-existent table: {0}")]
    NonExistentTableReference(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

fn kind_to_query_type(kind: &Kind) -> QueryType {
    match kind {
        Kind::Array(inner_kind, len) => {
            let inner_type = Box::new(TypedQuery {
                query_type: kind_to_query_type(inner_kind),
                perms: Permissions::none(),
            });
            let length_limit = len.and_then(|l| NonZeroU64::new(l as u64));
            QueryType::Array(Some(inner_type), length_limit)
        }
        Kind::Object => QueryType::Object(HashMap::new()),
        _ => QueryType::Scalar(kind.clone()),
    }
}

fn update_field(object: &mut QueryType, path: &[Part], field_type: QueryType, perms: Permissions) {
    if path.is_empty() {
        return;
    }

    match (object, &path[0]) {
        (QueryType::Object(fields), Part::Field(ident)) => {
            let field_name = ident.to_string().to_lowercase();
            if path.len() == 1 {
                fields.insert(
                    field_name,
                    TypedQuery {
                        query_type: field_type,
                        perms,
                    },
                );
            } else if path[1] == Part::All {
                // Handle array element definition (e.g., tags.*)
                let array_type = QueryType::Array(
                    Some(Box::new(TypedQuery {
                        query_type: field_type,
                        perms: perms.clone(),
                    })),
                    None,
                );
                fields.insert(
                    field_name,
                    TypedQuery {
                        query_type: array_type,
                        perms,
                    },
                );
            } else {
                let next = fields.entry(field_name).or_insert_with(|| TypedQuery {
                    query_type: QueryType::Object(HashMap::new()),
                    perms: Permissions::none(),
                });
                update_field(&mut next.query_type, &path[1..], field_type, perms);
            }
        }
        (QueryType::Array(inner, _), Part::All) => {
            *inner = Some(Box::new(TypedQuery {
                query_type: field_type,
                perms,
            }));
        }
        _ => {}
    }
}

pub fn parse_schema(schema: &str) -> Result<HashMap<String, TypedQuery>, SchemaParseError> {
    let query = surrealdb::sql::parse(schema)?;
    let mut tables: HashMap<String, TypedQuery> = HashMap::new();

    for statement in query.iter() {
        match statement {
            Statement::Define(DefineStatement::Table(table)) => {
                let table_name = table.name.to_string().to_lowercase();
                tables.insert(
                    table_name,
                    TypedQuery {
                        query_type: QueryType::Object(HashMap::new()),
                        perms: table.permissions.clone(),
                    },
                );
            }
            Statement::Define(DefineStatement::Field(field)) => {
                let table_name = field.what.to_string().to_lowercase();
                if let Some(table) = tables.get_mut(&table_name) {
                    if let QueryType::Object(ref mut fields) = table.query_type {
                        let field_type = if let Some(kind) = &field.kind {
                            kind_to_query_type(kind)
                        } else {
                            QueryType::Scalar(Kind::Any)
                        };

                        update_field(
                            &mut table.query_type,
                            &field.name.0,
                            field_type,
                            field.permissions.clone(),
                        );
                    }
                } else {
                    return Err(SchemaParseError::NonExistentTableReference(table_name));
                }
            }
            _ => {}
        }
    }

    Ok(tables)
}
