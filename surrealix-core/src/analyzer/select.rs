use crate::ast::{AstError, FieldInfo, FieldMetadata, ObjectType, ScalarType, TypeAST};
use std::collections::HashMap;
use surrealdb::sql::{
    statements::SelectStatement, Fetchs, Field, Fields, Idiom, Idioms, Part, Permissions, Value,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalyzeSelectError {
    #[error("Schema provided is not an object")]
    InvalidSchema,
    #[error("Unknown field: {0}")]
    UnknownField(String),
    #[error("Invalid field type")]
    InvalidFieldType,
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
    #[error(transparent)]
    AstError(#[from] AstError),
}

pub fn analyze_select(
    schema: &TypeAST,
    stmt: &SelectStatement,
) -> Result<TypeAST, AnalyzeSelectError> {
    let TypeAST::Object(schema_obj) = schema else {
        return Err(AnalyzeSelectError::InvalidSchema);
    };

    // Step 1: Analyze the 'FROM' clause
    let base_type = analyze_from(&schema_obj, &stmt.what)?;

    // Step 2: Apply field selection
    let mut selected_type = apply_field_selection(schema, &base_type, &stmt.expr, &stmt.omit)?;

    // Step 3: Apply fetch
    if let Some(fetch) = &stmt.fetch {
        for fetch_item in &fetch.0 {
            let fetched_ast = selected_type.resolve_idiom(&fetch_item.0)?;
            match fetched_ast {
                TypeAST::Record(_) => {
                    selected_type.replace_record_links(schema)?;
                }
                TypeAST::Array(boxed) => {
                    if let TypeAST::Record(_) = boxed.0 {
                        selected_type.replace_record_links(schema)?;
                    } else {
                        return Err(AnalyzeSelectError::UnsupportedOperation(format!(
                            "Unsupported fetch type: {:?}",
                            boxed.0
                        )));
                    }
                }
                _ => {
                    return Err(AnalyzeSelectError::UnsupportedOperation(format!(
                        "Unsupported fetch type: {:?}",
                        fetched_ast
                    )));
                }
            }
        }
    }

    // Step 4: Handle VALUE keyword
    let value_type = if stmt.expr.0.len() == 1 && stmt.expr.1 {
        // If there's only one field and VALUE keyword is used
        match &selected_type {
            TypeAST::Object(obj) => {
                if let Some(field) = obj.fields.values().next() {
                    field.ast.clone()
                } else {
                    return Err(AnalyzeSelectError::InvalidFieldType);
                }
            }
            _ => return Err(AnalyzeSelectError::InvalidFieldType),
        }
    } else {
        selected_type
    };

    // Step 5: Wrap in array if not ONLY
    let final_type = if stmt.only {
        value_type
    } else {
        TypeAST::Array(Box::new((value_type, None)))
    };

    Ok(final_type)
}

fn analyze_from(schema: &ObjectType, what: &[Value]) -> Result<TypeAST, AnalyzeSelectError> {
    if let Some(Value::Table(table)) = what.first() {
        schema
            .fields
            .get(&table.to_string())
            .map(|field_info| field_info.ast.clone())
            .ok_or_else(|| AnalyzeSelectError::UnknownField(table.to_string()))
    } else {
        Err(AnalyzeSelectError::UnsupportedOperation(
            "Unsupported FROM clause".to_string(),
        ))
    }
}

fn apply_field_selection(
    schema: &TypeAST,
    base_type: &TypeAST,
    expr: &Fields,
    omit: &Option<Idioms>,
) -> Result<TypeAST, AnalyzeSelectError> {
    let TypeAST::Object(base_obj) = base_type else {
        return Err(AnalyzeSelectError::InvalidFieldType);
    };

    let mut result_fields = HashMap::new();

    for field in &expr.0 {
        match field {
            Field::All => {
                // Include all fields except those in the OMIT clause
                for (name, field_info) in &base_obj.fields {
                    if !is_field_omitted(name, omit) {
                        result_fields.insert(name.clone(), field_info.clone());
                    }
                }
            }
            Field::Single { expr, alias } => match expr {
                Value::Idiom(idiom) => {
                    println!("Resolving graph traversal for idiom: {:?}", idiom);
                    let (field_name, field_ast) =
                        resolve_graph_traversal(schema, base_type, idiom)?;

                    let result_name = alias
                        .as_ref()
                        .map(|a| a.to_string())
                        .unwrap_or(field_name.clone());
                    if !is_field_omitted(&result_name, omit) {
                        result_fields.insert(
                            result_name,
                            FieldInfo {
                                ast: field_ast,
                                meta: FieldMetadata {
                                    original_name: field_name,
                                    original_path: idiom.0.iter().map(|p| p.to_string()).collect(),
                                    permissions: Permissions::default(),
                                },
                            },
                        );
                    }
                }
                _ => {
                    return Err(AnalyzeSelectError::UnsupportedOperation(
                        "Unsupported field expression".to_string(),
                    ))
                }
            },
        }
    }

    Ok(TypeAST::Object(ObjectType {
        fields: result_fields,
    }))
}

fn resolve_graph_traversal(
    schema: &TypeAST,
    base_type: &TypeAST,
    idiom: &Idiom,
) -> Result<(String, TypeAST), AnalyzeSelectError> {
    let mut current_type = base_type;
    let mut field_name = String::new();

    for (i, part) in idiom.0.iter().enumerate() {
        match part {
            Part::Field(ident) => {
                field_name = ident.to_string();
                if let TypeAST::Object(obj) = current_type {
                    if let Some(field_info) = obj.fields.get(&field_name) {
                        current_type = &field_info.ast;
                    } else {
                        println!("Encountered an unknown field in idiom: {:?}", field_name);
                        return Err(AnalyzeSelectError::UnknownField(field_name));
                    }
                } else {
                    return Err(AnalyzeSelectError::InvalidFieldType);
                }
            }
            Part::Graph(graph) => {
                let edge_table = &graph.what.0[0].to_string();
                field_name = format!("->{}", edge_table);

                if let TypeAST::Object(schema_obj) = schema {
                    if let Some(edge_table_info) = schema_obj.fields.get(edge_table) {
                        if let TypeAST::Object(edge_obj) = &edge_table_info.ast {
                            println!(
                                "Edge table '{}' fields: {:?}",
                                edge_table,
                                edge_obj.fields.keys().collect::<Vec<_>>()
                            );

                            let (relation_field, target_table) =
                                find_relation_field(edge_obj, &graph.dir)?;

                            println!("Found relation field: {}", relation_field);
                            println!("Target table: {}", target_table);

                            if let Some(target_table_info) = schema_obj.fields.get(&target_table) {
                                current_type = &target_table_info.ast;
                                field_name = format!("{}->{}.*", field_name, target_table);
                            } else {
                                return Err(AnalyzeSelectError::UnknownField(target_table.clone()));
                            }
                        } else {
                            return Err(AnalyzeSelectError::InvalidFieldType);
                        }
                    } else {
                        return Err(AnalyzeSelectError::UnknownField(edge_table.clone()));
                    }
                } else {
                    return Err(AnalyzeSelectError::InvalidSchema);
                }
            }
            Part::All if i == idiom.0.len() - 1 => {
                // We've reached the end of the traversal, return the current type
                return Ok((field_name, current_type.clone()));
            }
            _ => {
                return Err(AnalyzeSelectError::UnsupportedOperation(format!(
                    "Unsupported graph traversal part: {:?}",
                    part
                )))
            }
        }
    }

    Ok((field_name, current_type.clone()))
}

fn find_relation_field(
    edge_obj: &ObjectType,
    dir: &surrealdb::sql::Dir,
) -> Result<(String, String), AnalyzeSelectError> {
    let (primary, fallback) = match dir {
        surrealdb::sql::Dir::Out => ("out", "in"),
        surrealdb::sql::Dir::In => ("in", "out"),
        _ => {
            return Err(AnalyzeSelectError::UnsupportedOperation(
                "Unsupported graph direction".to_string(),
            ))
        }
    };

    if let Some(field) = edge_obj
        .fields
        .get(primary)
        .or_else(|| edge_obj.fields.get(fallback))
    {
        if let TypeAST::Record(target_table) = &field.ast {
            Ok((
                field.meta.original_name.to_string(),
                target_table.to_string(),
            ))
        } else {
            Err(AnalyzeSelectError::InvalidFieldType)
        }
    } else {
        Err(AnalyzeSelectError::UnknownField(format!(
            "Neither '{}' nor '{}' field found",
            primary, fallback
        )))
    }
}

fn is_field_omitted(field_name: &str, omit: &Option<Idioms>) -> bool {
    omit.as_ref().map_or(false, |idioms| {
        idioms.0.iter().any(|idiom| {
            idiom.0.first().map_or(
                false,
                |part| matches!(part, Part::Field(ident) if ident.to_string() == field_name),
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::{ScalarType, TypeAST},
        schema::analyze_schema,
    };
    use surrealdb::sql::{parse, Statement};

    fn create_test_schema() -> TypeAST {
        let schema = r#"
            DEFINE TABLE user SCHEMAFULL;
                DEFINE FIELD id on user TYPE uuid;
                DEFINE FIELD name ON user TYPE string;
                DEFINE FIELD age ON user TYPE number;
                DEFINE FIELD address on user TYPE object;
                    DEFINE FIELD address.city on user TYPE string;
                    DEFINE FIELD address.zip on user TYPE number;
                    DEFINE FIELD address.state on user TYPE string;
                    DEFINE FIELD address.street on user TYPE string;
                DEFINE FIELD tags on user TYPE array;
                    DEFINE FIELD tags.* on user TYPE record<tag>;
                DEFINE FIELD best_friend on user TYPE record<user>;
            DEFINE TABLE friend SCHEMAFULL;
                DEFINE FIELD in ON friend TYPE record<user>;
                DEFINE FIELD out ON friend TYPE record<user>;
            DEFINE TABLE tag SCHEMAFULL;
                DEFINE FIELD id on tag TYPE uuid;
                DEFINE FIELD name on tag TYPE string;
                DEFINE FIELD value on tag TYPE number;
        "#;

        let parsed = surrealdb::sql::parse(schema).unwrap();
        analyze_schema(parsed).unwrap()
    }

    fn parse_select(input: &str) -> SelectStatement {
        let query = parse(input).unwrap();
        match query.0.first().unwrap() {
            Statement::Select(stmt) => stmt.clone(),
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn select() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT id, name, age FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 3);
        assert!(obj.fields.contains_key("id"));
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("age"));
    }

    #[test]
    fn select_all() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT * FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 6);
        assert!(obj.fields.contains_key("id"));
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("age"));
        assert!(obj.fields.contains_key("address"));
        assert!(obj.fields.contains_key("tags"));
        assert!(obj.fields.contains_key("best_friend"));
    }

    #[test]
    fn select_one() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT * FROM ONLY user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Object(obj) = result else {
            panic!("Expected Object TypeAST");
        };

        assert_eq!(obj.fields.len(), 6);
        assert!(obj.fields.contains_key("id"));
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("age"));
        assert!(obj.fields.contains_key("address"));
        assert!(obj.fields.contains_key("tags"));
        assert!(obj.fields.contains_key("best_friend"));
    }

    #[test]
    fn select_rename() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT name AS full_name, age FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 2);
        assert!(obj.fields.contains_key("full_name"));
        assert!(obj.fields.contains_key("age"));
        assert_eq!(obj.fields["full_name"].meta.original_name, "name");
        assert!(matches!(
            obj.fields["full_name"].ast,
            TypeAST::Scalar(ScalarType::String)
        ));
    }

    #[test]
    fn select_omit() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT * OMIT age FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 5);
        assert!(obj.fields.contains_key("id"));
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("address"));
        assert!(obj.fields.contains_key("tags"));
        assert!(obj.fields.contains_key("best_friend"));

        //It should not contain age!
        assert!(!obj.fields.contains_key("age"));
    }

    #[test]
    fn select_object() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT address FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 1);
        assert!(obj.fields.contains_key("address"));
        let TypeAST::Object(address_obj) = &obj.fields["address"].ast else {
            panic!("Expected Object TypeAST for address");
        };
        assert!(address_obj.fields.contains_key("city"));
    }

    #[test]
    fn test_select_value() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT VALUE age FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Scalar(scalar_type) = boxed_arr.0 else {
            panic!("Expected Scalar TypeAST inside Array");
        };

        assert!(matches!(scalar_type, ScalarType::Number));
    }

    #[test]
    fn fetch_array() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT name, tags FROM user FETCH tags");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 2);
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("tags"));

        // Check that tags are fetched
        let TypeAST::Array(tag_boxed) = &obj.fields["tags"].ast else {
            panic!("Expected Array TypeAST for tags");
        };

        let TypeAST::Object(tag_obj) = &tag_boxed.0 else {
            panic!(
                "Expected Object inside Array for tags. Got: \n{:#?}",
                tag_boxed.0
            );
        };

        assert!(tag_obj.fields.contains_key("id"));
        assert!(tag_obj.fields.contains_key("name"));
        assert!(tag_obj.fields.contains_key("value"));
    }

    #[test]
    fn fetch_single() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT name, best_friend FROM user FETCH best_friend");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 2);
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("best_friend"));

        // Check that best_friend is fetched
        let TypeAST::Object(best_friend_obj) = &obj.fields["best_friend"].ast else {
            panic!("Expected Object TypeAST for best_friend");
        };

        assert!(best_friend_obj.fields.contains_key("id"));
        assert!(best_friend_obj.fields.contains_key("name"));
        assert!(best_friend_obj.fields.contains_key("age"));
        assert!(best_friend_obj.fields.contains_key("address"));
        assert!(best_friend_obj.fields.contains_key("tags"));
        assert!(best_friend_obj.fields.contains_key("best_friend"));
    }

    #[test]
    fn demo() {
        let schema = create_test_schema();
        let query = "SELECT name, age as renamedAge, address, ->friend->user.* FROM user;";
        let stmt = parse_select(query);
        let result = analyze_select(&schema, &stmt).unwrap();
    }
}
