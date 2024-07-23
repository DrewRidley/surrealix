use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_datatype(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts.get(1) {
        Some(&"bool") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Bool),
            perms: Permissions::none(),
        },
        Some(&"datetime") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Datetime),
            perms: Permissions::none(),
        },
        Some(&"decimal") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Decimal),
            perms: Permissions::none(),
        },
        Some(&"duration") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Duration),
            perms: Permissions::none(),
        },
        Some(&"float") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Float),
            perms: Permissions::none(),
        },
        Some(&"int") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Int),
            perms: Permissions::none(),
        },
        Some(&"number") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Number),
            perms: Permissions::none(),
        },
        Some(&"point") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Geometry(vec![])),
            perms: Permissions::none(),
        },
        Some(&"string") => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::none(),
        },
        Some(&"table") => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::none(),
        },
        Some(&"thing") => todo!("Implement 'thing'"),
        Some(&"range") => todo!("Implement range"),
        Some(&"field") | Some(&"fields") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
        Some(&"is") => match parts.get(2) {
            Some(_) => TypedQuery {
                query_type: QueryType::Scalar(Kind::Bool),
                perms: Permissions::none(),
            },
            None => TypedQuery {
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
