use std::{collections::HashMap, num::NonZeroU64};

use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_object(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts[1] {
        "entries" => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Array(
                        Some(Box::new(TypedQuery {
                            query_type: QueryType::Scalar(Kind::Any),
                            perms: Permissions::none(),
                        })),
                        NonZeroU64::new(2),
                    ),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
        "from_entries" => TypedQuery {
            query_type: QueryType::Object(HashMap::new()),
            perms: Permissions::none(),
        },
        "keys" => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Scalar(Kind::String),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
        "len" => TypedQuery {
            query_type: QueryType::Scalar(Kind::Int),
            perms: Permissions::none(),
        },
        "values" => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Scalar(Kind::Any),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
        _ => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
    }
}
