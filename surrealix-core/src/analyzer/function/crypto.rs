use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

pub fn analyze_crypto(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts[1] {
        "md5" | "sha1" | "sha256" | "sha512" => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::none(),
        },
        "argon2" | "bcrypt" | "pbkdf2" | "scrypt" => match parts[2] {
            "compare" => TypedQuery {
                query_type: QueryType::Scalar(Kind::Bool),
                perms: Permissions::none(),
            },
            "generate" => TypedQuery {
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
