use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_time(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts.get(1) {
        Some(&"day") | Some(&"hour") | Some(&"minute") | Some(&"month") | Some(&"second")
        | Some(&"wday") | Some(&"week") | Some(&"yday") | Some(&"year") | Some(&"micros")
        | Some(&"millis") | Some(&"nano") | Some(&"unix") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Int),
            perms: Permissions::none(),
        },
        Some(&"floor") | Some(&"round") | Some(&"group") | Some(&"now") | Some(&"max")
        | Some(&"min") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Datetime),
            perms: Permissions::none(),
        },
        Some(&"format") => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::none(),
        },
        Some(&"timezone") => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::none(),
        },
        Some(&"from") => match parts.get(2) {
            Some(&"micros") | Some(&"millis") | Some(&"nanos") | Some(&"secs") | Some(&"unix") => {
                TypedQuery {
                    query_type: QueryType::Scalar(Kind::Datetime),
                    perms: Permissions::none(),
                }
            }
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
