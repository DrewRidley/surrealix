use super::Tables;
use crate::types::{QueryType, TypedQuery};
use std::collections::HashMap;
use surrealdb::sql::{
    statements::SelectStatement, Field, Ident, Idiom, Kind, Part, Permissions, Value,
};

pub fn analyze_select(tbls: &Tables, statement: SelectStatement) -> TypedQuery {
    println!("Analyzing select statement: \n{:#?}\n", statement);

    let is_value = statement.expr.1;
    let fields = statement.expr.0;

    if is_value {
        // Handle SELECT VALUE statement
        if let Some(field) = fields.first() {
            let field_type = analyze_field(tbls, &statement.what, field);
            return TypedQuery {
                query_type: QueryType::Array(Some(Box::new(field_type)), None),
                perms: Permissions::none(),
            };
        }
    }

    let mut object_type = HashMap::new();

    for field in fields {
        match field {
            Field::All => {
                // Include all fields from the base table
                for what in statement.what.iter() {
                    if let Some(table) = tbls.get(&what.to_string()) {
                        if let QueryType::Object(table_fields) = &table.query_type {
                            object_type = table_fields.clone();
                        }
                    }
                }
            }
            Field::Single { expr, alias } => {
                if let Value::Idiom(idiom) = expr {
                    let field_type = analyze_idiom(tbls, &statement.what, &idiom);

                    let field_name = alias.clone().unwrap_or_else(|| {
                        // For graph traversals, use the last table name as the field name
                        let last_graph_table = idiom.iter().rev().find_map(|part| {
                            if let Part::Graph(graph) = part {
                                graph.what.first().cloned()
                            } else {
                                None
                            }
                        });

                        if let Some(table) = last_graph_table {
                            // Create a new Idiom with just the table name
                            Idiom(vec![Part::Field(Ident(table.to_string()))])
                        } else {
                            // If no graph part found, use the original idiom
                            idiom.clone()
                        }
                    });

                    // Convert Idiom to a string representation
                    let field_name_str = field_name
                        .iter()
                        .map(|part| part.to_string())
                        .collect::<Vec<_>>()
                        .join(".")
                        .trim_start_matches('.')
                        .to_string();

                    // Now we can use replace on the string representation
                    let clean_field_name = field_name_str.replace("[*]", "");

                    object_type.insert(clean_field_name, field_type);
                }
            }
        }
    }

    println!(
        "For SELECT query, constructed typed query: {:#?}",
        object_type
    );

    TypedQuery {
        query_type: QueryType::Array(
            Some(Box::new(TypedQuery {
                query_type: QueryType::Object(object_type),
                perms: Permissions::none(),
            })),
            None,
        ),
        perms: Permissions::none(),
    }
}

fn analyze_field(tbls: &Tables, table_names: &[Value], field: &Field) -> TypedQuery {
    match field {
        Field::All => TypedQuery {
            query_type: QueryType::Object(HashMap::new()),
            perms: Permissions::none(),
        },
        Field::Single { expr, .. } => {
            if let Value::Idiom(idiom) = expr {
                analyze_idiom(tbls, table_names, idiom)
            } else {
                TypedQuery {
                    query_type: QueryType::Scalar(Kind::Any),
                    perms: Permissions::none(),
                }
            }
        }
    }
}

fn analyze_idiom(tbls: &Tables, table_names: &[Value], idiom: &Idiom) -> TypedQuery {
    println!("Analyzing idiom: {:?}", idiom);
    println!("Available tables: {:?}", tbls.keys().collect::<Vec<_>>());

    let mut current_table = None;
    let mut field_parts = Vec::new();

    // Split idiom into graph traversals and field access
    for part in idiom.iter() {
        match part {
            surrealdb::sql::Part::Graph(graph) => {
                if let Some(table) = graph.what.first() {
                    current_table = Some(table.to_string());
                }
            }
            surrealdb::sql::Part::Field(field) => {
                field_parts.push(surrealdb::sql::Part::Field(field.clone()));
            }
            surrealdb::sql::Part::All => {
                field_parts.push(surrealdb::sql::Part::All);
            }
            // Handle other parts as needed
            _ => {
                println!("Unhandled part: {:?}", part);
                // You might want to add more specific handling for other part types
            }
        }
    }

    // If we have a current_table from graph traversal, use it; otherwise, use the original table_names
    let table_to_check = if let Some(table) = current_table {
        vec![Value::Table(surrealdb::sql::Table(table))]
    } else {
        table_names.to_vec()
    };

    for table_name in table_to_check {
        println!("Checking table: {:?}", table_name);
        if let Some(table) = tbls.get(&table_name.to_string()) {
            println!("Found table: {:?}", table_name);
            println!("Table query_type: {:?}", table.query_type);

            // Create a new Idiom with only the field parts
            let field_idiom = Idiom(field_parts.clone());

            let field_type = traverse_object(&table.query_type, &field_idiom);
            if field_type.query_type != QueryType::Scalar(Kind::Any) {
                return field_type;
            }
        } else {
            println!("Table not found: {:?}", table_name);
        }
    }

    println!("Falling back to Any");
    TypedQuery {
        query_type: QueryType::Scalar(Kind::Any),
        perms: Permissions::none(),
    }
}

fn traverse_object(query_type: &QueryType, idiom: &Idiom) -> TypedQuery {
    println!("Traversing object with idiom: {:?}", idiom);
    println!("Initial query_type: {:?}", query_type);

    let mut current_type = query_type;

    for (index, part) in idiom.iter().enumerate() {
        println!("Checking part: {:?}", part);
        match current_type {
            QueryType::Object(fields) => {
                println!(
                    "Current type is Object with fields: {:?}",
                    fields.keys().collect::<Vec<_>>()
                );
                let field_name = part.to_string().trim_start_matches('.').to_string();
                if field_name == "*" || field_name == "[*]" {
                    // If we encounter "*" or "[*]", return the current object type
                    return TypedQuery {
                        query_type: current_type.clone(),
                        perms: Permissions::none(),
                    };
                } else if let Some(field_type) = fields.get(&field_name) {
                    println!("Found field: {:?}", field_name);
                    current_type = &field_type.query_type;
                } else {
                    println!("Field not found: {:?}", field_name);
                    return TypedQuery {
                        query_type: QueryType::Scalar(Kind::Any),
                        perms: Permissions::none(),
                    };
                }
            }
            QueryType::Array(Some(item_type), _) => {
                println!("Current type is Array");
                current_type = &item_type.query_type;
            }
            _ => {
                println!("Current type is neither Object nor Array");
                return TypedQuery {
                    query_type: QueryType::Scalar(Kind::Any),
                    perms: Permissions::none(),
                };
            }
        }
    }

    println!("Final current_type: {:?}", current_type);
    TypedQuery {
        query_type: current_type.clone(),
        perms: Permissions::none(),
    }
}
