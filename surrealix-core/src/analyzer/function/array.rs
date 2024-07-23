use crate::types::{QueryType, TypedQuery};
use surrealdb::sql::{Function, Kind, Permissions};

// Helper function to get the inner type of an array
fn get_array_inner_type(query_type: &QueryType) -> Option<Box<QueryType>> {
    if let QueryType::Array(Some(inner), _) = query_type {
        Some(Box::new(inner.clone().query_type))
    } else {
        None
    }
}

// Functions that don't change the array type
fn array_identity(args: &[TypedQuery]) -> TypedQuery {
    args.first().cloned().unwrap_or(TypedQuery {
        query_type: QueryType::Array(None, None),
        perms: Permissions::none(),
    })
}

// Functions that return a boolean
fn array_to_bool(_args: &[TypedQuery]) -> TypedQuery {
    TypedQuery {
        query_type: QueryType::Scalar(Kind::Bool),
        perms: Permissions::none(),
    }
}

// Functions that return a number
fn array_to_number(_args: &[TypedQuery]) -> TypedQuery {
    TypedQuery {
        query_type: QueryType::Scalar(Kind::Int),
        perms: Permissions::none(),
    }
}

// Functions that return a string
fn array_to_string(_args: &[TypedQuery]) -> TypedQuery {
    TypedQuery {
        query_type: QueryType::Scalar(Kind::String),
        perms: Permissions::none(),
    }
}

// Special cases
fn array_at(args: &[TypedQuery]) -> TypedQuery {
    if let Some(arg) = args.first() {
        if let Some(inner) = get_array_inner_type(&arg.query_type) {
            return TypedQuery {
                query_type: *inner,
                perms: Permissions::none(),
            };
        }
    }
    TypedQuery {
        query_type: QueryType::Scalar(Kind::Any),
        perms: Permissions::none(),
    }
}

fn array_clump(args: &[TypedQuery]) -> TypedQuery {
    if let Some(arg) = args.first() {
        return TypedQuery {
            query_type: QueryType::Array(Some(Box::new(arg.clone())), None),
            perms: Permissions::none(),
        };
    }
    TypedQuery {
        query_type: QueryType::Array(None, None),
        perms: Permissions::none(),
    }
}

fn array_flatten(args: &[TypedQuery]) -> TypedQuery {
    if let Some(arg) = args.first() {
        if let Some(inner) = get_array_inner_type(&arg.query_type) {
            if let Some(inner_inner) = get_array_inner_type(&inner) {
                return TypedQuery {
                    query_type: QueryType::Array(
                        Some(Box::new(TypedQuery {
                            query_type: *inner_inner,
                            perms: Permissions::none(),
                        })),
                        None,
                    ),
                    perms: Permissions::none(),
                };
            }
        }
    }
    TypedQuery {
        query_type: QueryType::Array(None, None),
        perms: Permissions::none(),
    }
}

pub fn analyze_array(func: &Function, args: Vec<TypedQuery>) -> TypedQuery {
    match func.name().unwrap() {
        // Functions that don't change the array type
        "array::add" | "array::append" | "array::combine" | "array::concat"
        | "array::difference" | "array::distinct" | "array::group" | "array::insert"
        | "array::intersect" | "array::pop" | "array::prepend" | "array::push"
        | "array::remove" | "array::reverse" | "array::shuffle" | "array::sort"
        | "array::slice" | "array::transpose" | "array::union" => array_identity(&args),

        // Functions that return a boolean
        "array::all" | "array::any" => array_to_bool(&args),

        // Functions that return a number
        "array::len" | "array::find_index" => array_to_number(&args),

        // Functions that return a string
        "array::join" => array_to_string(&args),

        // Special cases
        "array::at" => array_at(&args),
        "array::clump" => array_clump(&args),
        "array::flatten" => array_flatten(&args),

        // Functions that might return the type of the array elements
        "array::first" | "array::last" | "array::max" | "array::min" => {
            if let Some(arg) = args.first() {
                if let Some(inner) = get_array_inner_type(&arg.query_type) {
                    return TypedQuery {
                        query_type: *inner,
                        perms: Permissions::none(),
                    };
                }
            }
            TypedQuery {
                query_type: QueryType::Scalar(Kind::Any),
                perms: Permissions::none(),
            }
        }

        // Default case for unknown functions
        _ => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
    }
}
