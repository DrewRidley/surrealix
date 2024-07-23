use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_rand(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts.get(1) {
        Some(&"bool") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Bool),
            perms: Permissions::none(),
        },
        Some(&"enum") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
        Some(&"float") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Float),
            perms: Permissions::none(),
        },
        Some(&"guid") => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::none(),
        },
        Some(&"int") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Int),
            perms: Permissions::none(),
        },
        Some(&"string") => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::none(),
        },
        Some(&"time") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Datetime),
            perms: Permissions::none(),
        },
        Some(&"uuid") => {
            if parts.get(2) == Some(&"v4") || parts.get(2) == Some(&"v7") {
                TypedQuery {
                    query_type: QueryType::Scalar(Kind::Uuid),
                    perms: Permissions::none(),
                }
            } else {
                TypedQuery {
                    query_type: QueryType::Scalar(Kind::Uuid),
                    perms: Permissions::none(),
                }
            }
        }
        Some(&"ulid") => TypedQuery {
            query_type: QueryType::Scalar(Kind::String), // Assuming ULID is represented as a string
            perms: Permissions::none(),
        },
        None => TypedQuery {
            query_type: QueryType::Scalar(Kind::Float),
            perms: Permissions::none(),
        },
        _ => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
    }
}
