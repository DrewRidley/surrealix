use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_duration(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts[1] {
        "days" | "hours" | "micros" | "millis" | "mins" | "nanos" | "secs" | "weeks" | "years" => {
            TypedQuery {
                query_type: QueryType::Scalar(Kind::Number),
                perms: Permissions::none(),
            }
        }
        "from" => match parts[2] {
            "days" | "hours" | "micros" | "millis" | "mins" | "nanos" | "secs" | "weeks" => {
                TypedQuery {
                    query_type: QueryType::Scalar(Kind::Duration),
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
