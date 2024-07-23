use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

mod array;
mod crypto;
mod datatype;
mod duration;
mod math;
mod object;
mod parse;
mod rand;
mod search;
mod string;
mod time;
mod vector;

pub fn analyze_function(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    let parts: Vec<&str> = func.name().unwrap().split("::").collect();

    match parts[0] {
        "array" => array::analyze_array(func, args),
        "crypto" => crypto::analyze_crypto(func, args),
        "duration" => duration::analyze_duration(func, args),
        "math" => math::analyze_math(func, args),
        "object" => object::analyze_object(func, args),
        "parse" => parse::analyze_parse(func, args),
        "rand" => rand::analyze_rand(func, args),
        "search" => search::analyze_search(func, args),
        "type" => datatype::analyze_datatype(func, args),
        "vector" => vector::analyze_vector(func, args),
        // as of now, all possible 'session' fns return a string always.
        "session" => TypedQuery {
            query_type: QueryType::Scalar(Kind::String),
            perms: Permissions::full(),
        },
        "sleep" => TypedQuery {
            query_type: QueryType::Scalar(Kind::Null),
            perms: Permissions::none(),
        },
        "string" => string::analyze_string(func, args),
        "time" => time::analyze_time(func, args),
        "meta" => match parts[1] {
            "id" => TypedQuery {
                query_type: QueryType::Scalar(Kind::String),
                perms: Permissions::none(),
            },
            "tb" => TypedQuery {
                query_type: QueryType::Scalar(Kind::String),
                perms: Permissions::none(),
            },
            _ => todo!("Got invalid query! Replace with proper error handling."),
        },
        "encoding" => match parts[1] {
            "base64" => match parts[2] {
                "encode" => TypedQuery {
                    query_type: QueryType::Scalar(Kind::String),
                    perms: Permissions::none(),
                },
                "decode" => TypedQuery {
                    query_type: QueryType::Scalar(Kind::Bytes),
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
        },
        "http" => match parts[1] {
            "head" => TypedQuery {
                query_type: QueryType::Scalar(Kind::Null),
                perms: Permissions::none(),
            },
            "get" | "put" | "post" | "patch" | "delete" => TypedQuery {
                query_type: QueryType::Scalar(Kind::Any),
                perms: Permissions::none(),
            },
            _ => TypedQuery {
                query_type: QueryType::Scalar(Kind::Any),
                perms: Permissions::none(),
            },
        },
        "count" => TypedQuery {
            query_type: QueryType::Scalar(Kind::Int),
            perms: Permissions::full(),
        },
        _ => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::full(),
        },
    }
}
