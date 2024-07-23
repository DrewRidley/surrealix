use std::collections::HashMap;

use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_search(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts.get(1) {
        Some(&"score") => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Scalar(Kind::Float),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
        Some(&"highlight") => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Scalar(Kind::String),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
        Some(&"offsets") => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Object(HashMap::new()),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
        _ => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Scalar(Kind::Any),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
    }
}
