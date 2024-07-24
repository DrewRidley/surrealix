use super::{function::analyze_function, Tables};
use crate::types::{QueryType, TypedQuery};
use std::collections::HashMap;
use surrealdb::sql::{
    statements::SelectStatement, Cast, Fetchs, Field, Function, Ident, Idiom, Kind, Part,
    Permissions, Subquery, Value,
};

pub fn analyze_select(tbls: &Tables, statement: &SelectStatement) -> TypedQuery {
    let is_value = statement.expr.1;
    let fields = &statement.expr.0;

    if is_value {
        // Handle SELECT VALUE statement
        if let Some(field) = fields.first() {
            //The type of the array contents.
            let array_type = analyze_field(tbls, &statement.what, field);

            return TypedQuery {
                // 'SELECT VALUE' returns an array.
                // TODO: handle nuanced case where a 'DISTINCT' value can be selected.
                query_type: QueryType::Array(
                    Some(Box::new(TypedQuery {
                        query_type: array_type.query_type,
                        perms: array_type.perms.clone(),
                    })),
                    None,
                ),
                //Array cant be mutated directly so 'full' permissions are inferred.
                perms: Permissions::full(),
            };
        }
    }

    let mut object_type = HashMap::new();

    for field in fields {
        match field {
            Field::All => {
                // Include all fields from the base table or subquery
                object_type.extend(analyze_what(tbls, &statement.what));
            }
            Field::Single { expr, alias } => {
                let field_type = analyze_value(tbls, &statement.what, expr);
                let field_name = alias.clone().unwrap_or_else(|| expr.to_idiom());
                object_type.insert(field_name.to_string(), field_type);
            }
        }
    }

    let mut result = TypedQuery {
        query_type: QueryType::Array(
            Some(Box::new(TypedQuery {
                query_type: QueryType::Object(object_type),
                perms: Permissions::none(),
            })),
            None,
        ),
        perms: Permissions::full(),
    };

    if let Some(fetch) = &statement.fetch {
        result = apply_fetch(tbls, result, fetch);
    }

    result
}

fn apply_fetch(tbls: &Tables, mut query: TypedQuery, fetch: &Fetchs) -> TypedQuery {
    if let QueryType::Array(Some(inner), _) = &mut query.query_type {
        if let QueryType::Object(fields) = &mut inner.query_type {
            for fetch_item in fetch.iter() {
                let idiom = fetch_item.0.clone();
                if let Some(field_type) = traverse_and_fetch(tbls, fields, &idiom) {
                    fields.insert(idiom.to_string(), field_type);
                }
            }
        }
    }
    query
}

fn traverse_and_fetch(
    tbls: &Tables,
    fields: &mut HashMap<String, TypedQuery>,
    idiom: &Idiom,
) -> Option<TypedQuery> {
    let parts = &idiom.0;
    let mut current_fields = fields;

    for (i, part) in parts.iter().enumerate() {
        match part {
            Part::Field(field) => {
                let field_name = field.to_string();
                if i == parts.len() - 1 {
                    // We're at the last part of the idiom
                    return current_fields
                        .get(&field_name)
                        .map(|field_type| fetch_field(tbls, field_type));
                } else {
                    // We need to go deeper
                    if let Some(TypedQuery {
                        query_type: QueryType::Object(nested_fields),
                        ..
                    }) = current_fields.get_mut(&field_name)
                    {
                        current_fields = nested_fields;
                    } else {
                        return None;
                    }
                }
            }
            _ => return None, // Handle other Part variants if needed
        }
    }
    None
}

fn fetch_field(tbls: &Tables, field_type: &TypedQuery) -> TypedQuery {
    match &field_type.query_type {
        QueryType::Scalar(Kind::Record(table_names)) => {
            if let Some(table_name) = table_names.first() {
                if let Some(table_type) = tbls.get(&table_name.to_string()) {
                    return table_type.clone();
                }
            }
            field_type.clone()
        }
        QueryType::Array(Some(inner), _) => {
            if let QueryType::Scalar(Kind::Record(table_names)) = &inner.query_type {
                if let Some(table_name) = table_names.first() {
                    if let Some(table_type) = tbls.get(&table_name.to_string()) {
                        return TypedQuery {
                            query_type: QueryType::Array(Some(Box::new(table_type.clone())), None),
                            perms: field_type.perms.clone(),
                        };
                    }
                }
            }
            field_type.clone()
        }
        _ => field_type.clone(),
    }
}

fn analyze_what(tbls: &Tables, what: &[Value]) -> HashMap<String, TypedQuery> {
    let mut result = HashMap::new();
    for value in what {
        match value {
            Value::Table(table) => {
                if let Some(table_type) = tbls.get(&table.to_string()) {
                    if let QueryType::Object(fields) = &table_type.query_type {
                        result.extend(fields.clone());
                    }
                }
            }
            Value::Subquery(subquery) => {
                if let Subquery::Select(select) = &**subquery {
                    let subquery_type = analyze_select(tbls, select);
                    if let QueryType::Array(Some(inner), _) = subquery_type.query_type {
                        if let QueryType::Object(fields) = inner.query_type {
                            result.extend(fields);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    result
}

fn analyze_field(tbls: &Tables, table_names: &[Value], field: &Field) -> TypedQuery {
    match field {
        Field::All => TypedQuery {
            query_type: QueryType::Object(analyze_what(tbls, table_names)),
            perms: Permissions::none(),
        },
        Field::Single { expr, .. } => analyze_value(tbls, table_names, expr),
    }
}

fn analyze_value(tbls: &Tables, table_names: &[Value], value: &Value) -> TypedQuery {
    match value {
        Value::Idiom(idiom) => analyze_idiom(tbls, table_names, idiom),
        Value::Function(func) => {
            let args: Vec<TypedQuery> = func
                .args()
                .iter()
                .map(|arg| analyze_value(tbls, table_names, arg))
                .collect();
            analyze_function(func, args)
        }
        Value::Subquery(subquery) => {
            if let Subquery::Select(select) = &**subquery {
                analyze_select(tbls, select)
            } else {
                TypedQuery {
                    query_type: QueryType::Scalar(Kind::Any),
                    perms: Permissions::none(),
                }
            }
        }
        Value::Cast(cast) => TypedQuery {
            query_type: QueryType::Scalar(cast.0.clone()),
            perms: Permissions::none(),
        },
        Value::Param(_) => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
        Value::Constant(_) => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
        _ => TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        },
    }
}

fn analyze_idiom(tbls: &Tables, table_names: &[Value], idiom: &Idiom) -> TypedQuery {
    let mut current_type = None;

    for table_name in table_names {
        if let Value::Table(table) = table_name {
            if let Some(table_type) = tbls.get(&table.to_string()) {
                current_type = Some(table_type);
                break;
            }
        }
    }

    if let Some(start_type) = current_type {
        traverse_idiom(tbls, &start_type.query_type, idiom)
    } else {
        TypedQuery {
            query_type: QueryType::Scalar(Kind::Any),
            perms: Permissions::none(),
        }
    }
}

fn traverse_idiom(tbls: &Tables, query_type: &QueryType, idiom: &Idiom) -> TypedQuery {
    let mut current_type = query_type;

    for part in idiom.0.iter() {
        match (current_type, part) {
            (QueryType::Object(fields), Part::Field(Ident(field_name))) => {
                if let Some(field_type) = fields.get(field_name) {
                    current_type = &field_type.query_type;
                } else {
                    return TypedQuery {
                        query_type: QueryType::Scalar(Kind::Any),
                        perms: Permissions::none(),
                    };
                }
            }
            (QueryType::Array(Some(inner_type), _), Part::All) => {
                current_type = &inner_type.query_type;
            }
            (_, Part::Graph(graph)) => {
                if let Some(table_name) = graph.what.first() {
                    if let Some(table_type) = tbls.get(&table_name.to_string()) {
                        current_type = &table_type.query_type;
                    } else {
                        return TypedQuery {
                            query_type: QueryType::Scalar(Kind::Any),
                            perms: Permissions::none(),
                        };
                    }
                }
            }
            (QueryType::Object(_), Part::All) => {
                // Return the entire object when selecting all fields
                return TypedQuery {
                    query_type: current_type.clone(),
                    perms: Permissions::none(),
                };
            }
            _ => {
                return TypedQuery {
                    query_type: QueryType::Scalar(Kind::Any),
                    perms: Permissions::none(),
                };
            }
        }
    }

    TypedQuery {
        query_type: current_type.clone(),
        perms: Permissions::none(),
    }
}
