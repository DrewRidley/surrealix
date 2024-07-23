use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_parse(func: &Function, _args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match (parts[1], parts[2]) {
        ("email", "host") | ("email", "user") => TypedQuery {
            query_type: QueryType::Option(Box::new(TypedQuery {
                query_type: QueryType::Scalar(Kind::String),
                perms: Permissions::full(),
            })),
            perms: Permissions::none(),
        },
        ("url", "domain")
        | ("url", "fragment")
        | ("url", "host")
        | ("url", "path")
        | ("url", "query") => TypedQuery {
            query_type: QueryType::Option(Box::new(TypedQuery {
                query_type: QueryType::Scalar(Kind::String),
                perms: Permissions::full(),
            })),
            perms: Permissions::none(),
        },
        ("url", "port") => TypedQuery {
            query_type: QueryType::Option(Box::new(TypedQuery {
                query_type: QueryType::Scalar(Kind::Int),
                perms: Permissions::full(),
            })),
            perms: Permissions::none(),
        },
        _ => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
    }
}
