use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_string(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts.get(1) {
        Some(&"concat") | Some(&"join") | Some(&"lowercase") | Some(&"repeat")
        | Some(&"replace") | Some(&"reverse") | Some(&"slice") | Some(&"slug") | Some(&"trim")
        | Some(&"uppercase") => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::none(),
        },
        Some(&"contains") | Some(&"endsWith") | Some(&"startsWith") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Bool),
            perms: Permissions::none(),
        },
        Some(&"len") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Int),
            perms: Permissions::none(),
        },
        Some(&"split") | Some(&"words") => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Scalar(Kind::String),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
        Some(&"is") => match parts.get(2) {
            Some(&"alphanum") | Some(&"alpha") | Some(&"ascii") | Some(&"datetime")
            | Some(&"domain") | Some(&"email") | Some(&"hexadecimal") | Some(&"latitude")
            | Some(&"longitude") | Some(&"numeric") | Some(&"semver") | Some(&"url")
            | Some(&"uuid") => TypedQuery {
                query_type: QueryType::Scalar(Kind::Bool),
                perms: Permissions::none(),
            },
            _ => TypedQuery {
                query_type: QueryType::Scalar(Kind::Any),
                perms: Permissions::none(),
            },
        },
        Some(&"semver") => match parts.get(2) {
            Some(&"compare") => TypedQuery {
                query_type: QueryType::Scalar(Kind::Int),
                perms: Permissions::none(),
            },
            Some(&"major") | Some(&"minor") | Some(&"patch") => TypedQuery {
                query_type: QueryType::Scalar(Kind::Int),
                perms: Permissions::none(),
            },
            Some(&"inc") | Some(&"set") => TypedQuery {
                query_type: QueryType::Scalar(Kind::String),
                perms: Permissions::none(),
            },
            _ => TypedQuery {
                query_type: QueryType::Scalar(Kind::Any),
                perms: Permissions::none(),
            },
        },
        _ => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
    }
}
