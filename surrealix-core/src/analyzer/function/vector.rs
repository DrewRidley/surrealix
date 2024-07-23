use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_vector(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts.get(1) {
        Some(&"add") | Some(&"cross") | Some(&"divide") | Some(&"multiply")
        | Some(&"normalize") | Some(&"project") | Some(&"subtract") => TypedQuery {
            query_type: QueryType::Array(
                Some(Box::new(TypedQuery {
                    query_type: QueryType::Scalar(Kind::Number),
                    perms: Permissions::none(),
                })),
                None,
            ),
            perms: Permissions::none(),
        },
        Some(&"angle") | Some(&"dot") | Some(&"magnitude") => TypedQuery {
            query_type: QueryType::Scalar(Kind::Float),
            perms: Permissions::none(),
        },
        Some(&"distance") => match parts.get(2) {
            Some(&"chebyshev") | Some(&"euclidean") | Some(&"hamming") | Some(&"manhattan")
            | Some(&"minkowski") => TypedQuery {
                query_type: QueryType::Scalar(Kind::Float),
                perms: Permissions::none(),
            },
            _ => TypedQuery {
                query_type: QueryType::Scalar(Kind::Any),
                perms: Permissions::none(),
            },
        },
        Some(&"similarity") => match parts.get(2) {
            Some(&"cosine") | Some(&"jaccard") | Some(&"pearson") => TypedQuery {
                query_type: QueryType::Scalar(Kind::Float),
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
