use crate::{
    ast::{FieldInfo, FieldMetadata, ObjectType, ResolverError, ScalarType, TypeAST},
    errors::AnalysisError,
};
use std::collections::HashMap;
use surrealdb::sql::{
    statements::SelectStatement, Fetchs, Field, Fields, Idiom, Idioms, Part, Permissions, Value,
};
use thiserror::Error;
pub fn analyze_select(schema: &TypeAST, stmt: &SelectStatement) -> Result<TypeAST, AnalysisError> {
    let TypeAST::Object(schema_obj) = schema else {
        return Err(AnalysisError::UnsupportedType(format!(
            "Schema was not an object! This should not be possible. Please file a bug report."
        )));
    };

    let base_type = analyze_from(&schema_obj, &stmt.what)?;

    let mut selected_type = apply_field_selection(schema, &base_type, &stmt.expr, &stmt.omit)
        .map_err(|e| AnalysisError::UnsupportedOperation(e.to_string()))?;

    if let Some(fetch) = &stmt.fetch {
        for fetch_item in &fetch.0 {
            let fetched_ast = selected_type
                .resolve_idiom(&fetch_item.0)
                .map_err(|e| AnalysisError::ResolverFailure(e))?;
            match fetched_ast {
                TypeAST::Record(_) => {
                    selected_type
                        .replace_record_links(schema)
                        .map_err(|e| AnalysisError::ResolverFailure(e))?;
                }
                TypeAST::Array(boxed) => {
                    if let TypeAST::Record(_) = boxed.0 {
                        selected_type.replace_record_links(schema)?;
                    } else {
                        return Err(AnalysisError::UnsupportedOperation(format!(
                            "Unsupported fetch type: {:?}",
                            boxed.0
                        )));
                    }
                }
                _ => {
                    return Err(AnalysisError::UnsupportedOperation(format!(
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
                    match &field.ast {
                        TypeAST::Array(boxed) => (*boxed).0.clone(),
                        _ => field.ast.clone(),
                    }
                } else {
                    return Err(AnalysisError::UnsupportedType(format!(
                        "'VALUE' cannot be used on an empty object!"
                    )));
                }
            }
            _ => {
                return Err(AnalysisError::UnsupportedType(format!(
                    "'VALUE' cannot select from a non-table type."
                )))
            }
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

fn analyze_from(schema: &ObjectType, what: &[Value]) -> Result<TypeAST, AnalysisError> {
    if let Some(Value::Table(table)) = what.first() {
        schema
            .fields
            .get(&table.to_string().to_lowercase())
            .map(|field_info| field_info.ast.clone())
            .ok_or_else(|| AnalysisError::UnknownField(table.to_string()))
    } else {
        Err(AnalysisError::UnsupportedOperation(
            "Unsupported FROM clause".to_string(),
        ))
    }
}

fn apply_field_selection(
    schema: &TypeAST,
    base_type: &TypeAST,
    expr: &Fields,
    omit: &Option<Idioms>,
) -> Result<TypeAST, AnalysisError> {
    let TypeAST::Object(base_obj) = base_type else {
        return Err(AnalysisError::UnsupportedType(format!(
            "Selected from a non-object type!"
        )));
    };

    // Extract the table name from the base_type
    let table_name = base_obj
        .fields
        .values()
        .next()
        .and_then(|field| field.meta.original_path.first().cloned())
        .unwrap_or_else(|| "unknown".to_string());

    let mut result_fields = HashMap::new();

    for field in &expr.0 {
        match field {
            Field::All => {
                // Include all fields except those in the OMIT clause
                for (name, field_info) in &base_obj.fields {
                    if !is_field_omitted(name, omit) {
                        let mut new_field_info = field_info.clone();
                        new_field_info
                            .meta
                            .original_path
                            .insert(0, table_name.clone());
                        result_fields.insert(name.clone(), new_field_info);
                    }
                }
            }
            Field::Single { expr, alias } => match expr {
                Value::Idiom(idiom) => {
                    let (field_name, field_ast) =
                        resolve_graph_traversal(schema, base_type, idiom)?;

                    let result_name = alias.as_ref().map(|a| a.to_string()).unwrap_or_else(|| {
                        if field_name.starts_with("->") || field_name.starts_with("<-") {
                            field_name
                                .split("->")
                                .last()
                                .unwrap_or(&field_name)
                                .to_string()
                        } else {
                            field_name.clone()
                        }
                    });

                    if !is_field_omitted(&result_name, omit) {
                        let mut original_path = vec![table_name.clone()];
                        original_path.extend(idiom.0.iter().map(|p| p.to_string()));
                        let field_info = FieldInfo {
                            ast: field_ast,
                            meta: FieldMetadata {
                                original_name: field_name.clone(),
                                original_path,
                                permissions: Permissions::default(),
                            },
                        };

                        result_fields.insert(result_name, field_info);
                    }
                }
                _ => {
                    return Err(AnalysisError::UnsupportedOperation(
                        "Unsupported field expression".to_string(),
                    ));
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
) -> Result<(String, TypeAST), AnalysisError> {
    let mut current_type = base_type;
    let mut field_name = String::new();
    let mut traversal_path = Vec::new();

    for (i, part) in idiom.0.iter().enumerate() {
        match part {
            Part::Field(ident) => {
                field_name = ident.to_string();
                match current_type {
                    TypeAST::Object(obj) => {
                        if let Some(field_info) = obj.fields.get(&field_name) {
                            current_type = &field_info.ast;
                            traversal_path.push(field_name.clone());
                        } else {
                            return Err(AnalysisError::UnknownField(field_name));
                        }
                    }
                    TypeAST::Array(boxed) => {
                        // Handle array types
                        current_type = &boxed.0;
                        traversal_path.push(field_name.clone());
                    }
                    TypeAST::Record(record_type) => {
                        // Handle record type by looking up the field in the schema
                        if let TypeAST::Object(schema_obj) = schema {
                            if let Some(record_info) = schema_obj.fields.get(record_type) {
                                if let TypeAST::Object(record_obj) = &record_info.ast {
                                    if let Some(field_info) = record_obj.fields.get(&field_name) {
                                        current_type = &field_info.ast;
                                        traversal_path.push(field_name.clone());
                                    } else {
                                        return Err(AnalysisError::UnknownField(field_name));
                                    }
                                } else {
                                    return Err(AnalysisError::UnsupportedType(format!(
                                        "Got non-object where an object was expected in graph traversal!"
                                    )));
                                }
                            } else {
                                return Err(AnalysisError::UnknownField(record_type.clone()));
                            }
                        } else {
                            return Err(AnalysisError::UnsupportedOperation(format!("Found a record link to a non-object type. The Schema is likely invalid.")));
                        }
                    }
                    _ => {
                        return Err(AnalysisError::UnsupportedType(format!(
                            "Graph traversal encountered invalid type."
                        )));
                    }
                }
            }
            Part::Graph(graph) => {
                let edge_table = &graph.what.0[0].to_string();
                field_name = match graph.dir {
                    surrealdb::sql::Dir::Out => format!("->{}", edge_table),
                    surrealdb::sql::Dir::In => format!("<-{}", edge_table),
                    _ => {
                        return Err(AnalysisError::UnsupportedOperation(
                            "Unsupported graph direction".to_string(),
                        ))
                    }
                };
                traversal_path.push(field_name.clone());

                if let TypeAST::Object(schema_obj) = schema {
                    if let Some(edge_table_info) = schema_obj.fields.get(edge_table) {
                        if let TypeAST::Object(edge_obj) = &edge_table_info.ast {
                            let (relation_field, target_table) =
                                find_relation_field(edge_obj, &graph.dir)?;

                            if let Some(target_table_info) = schema_obj.fields.get(&target_table) {
                                current_type = &target_table_info.ast;
                                if relation_field != "id" {
                                    traversal_path.push(relation_field);
                                }
                                traversal_path.push(target_table.clone());
                            } else {
                                return Err(AnalysisError::UnknownField(target_table.clone()));
                            }
                        } else {
                            return Err(AnalysisError::UnsupportedType(format!(
                                "Edge table of graph traversal is not an object!"
                            )));
                        }
                    } else {
                        return Err(AnalysisError::UnknownField(edge_table.clone()));
                    }
                } else {
                    return Err(AnalysisError::UnsupportedType(format!(
                        "Schema is not an object!"
                    )));
                }
            }
            Part::All if i == idiom.0.len() - 1 => {
                // We've reached the end of the traversal, return the current type
                traversal_path.push("*".to_string());
                return Ok((
                    traversal_path.join("->"),
                    TypeAST::Array(Box::new((current_type.clone(), None))),
                ));
            }
            _ => {
                return Err(AnalysisError::UnsupportedOperation(format!(
                    "Unsupported graph traversal part: {:?}",
                    part
                )))
            }
        }
    }

    // If we've reached here, it's a regular field selection or a graph traversal without a wildcard
    let final_type = if traversal_path.len() > 1 {
        // It's a graph traversal, so wrap it in an array
        TypeAST::Array(Box::new((current_type.clone(), None)))
    } else {
        // It's a regular field selection, return as is
        current_type.clone()
    };

    Ok((traversal_path.join("->"), final_type))
}

fn find_relation_field(
    edge_obj: &ObjectType,
    dir: &surrealdb::sql::Dir,
) -> Result<(String, String), AnalysisError> {
    // Handle the case when dealing with the user table
    if edge_obj.fields.contains_key("id") {
        return Ok(("id".to_string(), "user".to_string()));
    }

    let (primary, fallback) = match dir {
        surrealdb::sql::Dir::Out => ("out", "in"),
        surrealdb::sql::Dir::In => ("in", "out"),
        _ => {
            return Err(AnalysisError::UnsupportedOperation(
                "Unsupported graph direction".to_string(),
            ))
        }
    };

    let primary_field = edge_obj.fields.get(primary);
    let fallback_field = edge_obj.fields.get(fallback);

    match (primary_field, fallback_field) {
        (Some(field), _) | (None, Some(field)) => {
            if let TypeAST::Record(target_table) = &field.ast {
                Ok((
                    field.meta.original_name.to_string(),
                    target_table.to_string(),
                ))
            } else {
                Err(AnalysisError::UnsupportedType(format!(
                    "Expected a record link but found other type."
                )))
            }
        }
        (None, None) => Err(AnalysisError::UnknownField(format!(
            "Neither '{}' nor '{}' field found in edge object",
            primary, fallback
        ))),
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
    fn test_graph_traversal_out() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT name, ->friend->user.name as friend_names FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 2);
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("friend_names"));

        let TypeAST::Array(friends_arr) = &obj.fields["friend_names"].ast else {
            panic!("Expected Array TypeAST for friend_names");
        };

        assert!(matches!(friends_arr.0, TypeAST::Scalar(ScalarType::String)));
    }

    #[test]
    fn test_graph_traversal_in() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT name, <-friend<-user.name as follower_names FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 2);
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("follower_names"));

        let TypeAST::Array(followers_arr) = &obj.fields["follower_names"].ast else {
            panic!("Expected Array TypeAST for follower_names");
        };

        assert!(matches!(
            followers_arr.0,
            TypeAST::Scalar(ScalarType::String)
        ));
    }

    #[test]
    fn test_graph_traversal_multi_hop() {
        let schema = create_test_schema();
        let stmt = parse_select(
            "SELECT name, ->friend->user->friend->user.name as friend_of_friend_names FROM user",
        );

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 2);
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("friend_of_friend_names"));

        let TypeAST::Array(fof_arr) = &obj.fields["friend_of_friend_names"].ast else {
            panic!("Expected Array TypeAST for friend_of_friend_names");
        };

        assert!(matches!(fof_arr.0, TypeAST::Scalar(ScalarType::String)));
    }

    #[test]
    fn test_graph_traversal() {
        let schema = create_test_schema();
        let stmt = parse_select("SELECT name, ->friend->user.* as friends FROM user");

        let result = analyze_select(&schema, &stmt).unwrap();

        let TypeAST::Array(boxed_arr) = result else {
            panic!("Expected Array TypeAST");
        };

        let TypeAST::Object(obj) = boxed_arr.0 else {
            panic!("Expected Object inside Array");
        };

        assert_eq!(obj.fields.len(), 2);
        assert!(obj.fields.contains_key("name"));
        assert!(obj.fields.contains_key("friends"));

        let TypeAST::Array(friends_arr) = &obj.fields["friends"].ast else {
            panic!("Expected Array TypeAST for friends");
        };

        let TypeAST::Object(friends_obj) = &friends_arr.0 else {
            panic!("Expected Object inside Array for friends");
        };

        // Check that the friends object contains user fields
        assert!(friends_obj.fields.contains_key("id"));
        assert!(friends_obj.fields.contains_key("name"));
        assert!(friends_obj.fields.contains_key("age"));
        assert!(friends_obj.fields.contains_key("address"));
        assert!(friends_obj.fields.contains_key("tags"));
        assert!(friends_obj.fields.contains_key("best_friend"));
    }
}
