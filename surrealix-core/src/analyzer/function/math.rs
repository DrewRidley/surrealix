use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_math(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts[1] {
        // Constants
        "e" | "pi" | "tau" | "inf" => TypedQuery {
            query_type: QueryType::Scalar(Kind::Number),
            perms: Permissions::none(),
        },

        // Functions that return a number
        "abs" | "ceil" | "floor" | "round" | "sqrt" | "fixed" => TypedQuery {
            query_type: QueryType::Scalar(Kind::Number),
            perms: Permissions::none(),
        },

        // Functions that take an array and return a number
        "max" | "min" | "mean" | "median" | "mode" | "product" | "sum" | "interquartile"
        | "midhinge" | "spread" | "stddev" | "trimean" | "variance" => TypedQuery {
            query_type: QueryType::Scalar(Kind::Number),
            perms: Permissions::none(),
        },

        // Functions that take an array and a number and return a number
        "percentile" | "nearestrank" => TypedQuery {
            query_type: QueryType::Scalar(Kind::Number),
            perms: Permissions::none(),
        },

        // Functions that return an array
        "bottom" | "top" => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Scalar(Kind::Number),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },

        // Default case
        _ => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
    }
}
