use surrealdb::sql::{
    statements::{
        DefineFieldStatement, DefineParamStatement, DefineStatement, DefineTableStatement,
    },
    Kind, Query, Statement,
};
use thiserror::Error;

use crate::ast::{FieldInfo, FieldMetadata, ObjectType, ScalarType, TypeAST};

#[derive(Error, Debug)]
pub enum SchemaParseError {
    #[error("Invalid SurrealQL syntax: {0}")]
    InvalidSyntax(#[from] surrealdb::error::Db),

    #[error("Reference to non-existent table: {0}")]
    NonExistentTableReference(String),

    #[error("Nested field '{0}' has no parent object")]
    MissingParentObject(String),

    #[error("Attempted to use '*' selector on non-array field '{0}'")]
    NonArrayStarSelector(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Provided a schema, generates a [TypeAST] encompassing all of the type info in the schema.
///
/// The returned [TypeAST] will *always* be an object, with the names of the tables as identifiers.
pub fn analyze_schema(schema: Query) -> Result<TypeAST, SchemaParseError> {
    let mut ast = TypeAST::Object(ObjectType::default());

    let mut field_definitions = vec![];

    for stmt in schema.iter() {
        match stmt {
            Statement::Define(DefineStatement::Field(def)) => field_definitions.push(def),
            Statement::Define(def) => apply_definition(def, &mut ast)?,
            _ => (),
        }
    }

    field_definitions.sort_by(|a, b| {
        let a_depth = a.name.0.len();
        let b_depth = b.name.0.len();
        a_depth.cmp(&b_depth)
    });

    for definition in field_definitions {
        apply_field_definition(definition, &mut ast)?;
    }

    Ok(ast)
}

/// Applies the specified table definition to an existing AST.
fn apply_definition(def: &DefineStatement, ast: &mut TypeAST) -> Result<(), SchemaParseError> {
    match def {
        DefineStatement::Table(table_def) => apply_table_definition(table_def, ast),
        DefineStatement::Param(param_def) => apply_param_definition(param_def, ast),
        DefineStatement::Event(_)
        | DefineStatement::Index(_)
        | DefineStatement::User(_)
        | DefineStatement::Model(_)
        | DefineStatement::Namespace(_)
        | DefineStatement::Database(_)
        | DefineStatement::Function(_)
        | DefineStatement::Analyzer(_)
        | DefineStatement::Token(_)
        | DefineStatement::Scope(_) => Ok(()),
        DefineStatement::Field(_) => Err(SchemaParseError::Unknown(
            "Received field definition in invalid location!".to_string(),
        )),
    }
}

fn apply_table_definition(
    table_def: &DefineTableStatement,
    ast: &mut TypeAST,
) -> Result<(), SchemaParseError> {
    let TypeAST::Object(schema) = ast else {
        return Err(SchemaParseError::Unknown(
            "Root AST is not an object".to_string(),
        ));
    };

    let table_name = table_def.name.to_string();
    let table_def = FieldInfo {
        ast: TypeAST::Object(ObjectType::default()),
        meta: FieldMetadata {
            original_name: table_name.clone(),
            original_path: vec![table_name.clone()],
            permissions: table_def.permissions.clone(),
        },
    };

    schema.fields.insert(table_name, table_def);
    Ok(())
}

fn apply_field_definition(
    field_def: &DefineFieldStatement,
    ast: &mut TypeAST,
) -> Result<(), SchemaParseError> {
    let TypeAST::Object(schema) = ast else {
        return Err(SchemaParseError::Unknown(
            "Root AST is not an object".to_string(),
        ));
    };

    let table_name = field_def.what.as_str().to_lowercase();
    let mut curr = schema
        .fields
        .get_mut(&table_name)
        .ok_or_else(|| SchemaParseError::NonExistentTableReference(field_def.what.to_string()))?;

    let parts = &field_def.name.0;
    let mut current_path = vec![table_name.clone()];

    for part in &parts[..parts.len() - 1] {
        match part {
            surrealdb::sql::Part::Field(ident) => {
                let field_name = ident.to_string();
                current_path.push(field_name.clone());
                match &mut curr.ast {
                    TypeAST::Object(obj) => {
                        curr = obj
                            .fields
                            .entry(field_name.clone())
                            .or_insert_with(|| FieldInfo {
                                ast: TypeAST::Object(ObjectType::default()),
                                meta: FieldMetadata {
                                    original_name: field_name.clone(),
                                    original_path: current_path.clone(),
                                    permissions: field_def.permissions.clone(),
                                },
                            });
                    }
                    _ => return Err(SchemaParseError::MissingParentObject(field_name)),
                }
            }
            _ => {
                return Err(SchemaParseError::Unknown(
                    "Unexpected part type in field path".to_string(),
                ))
            }
        }
    }

    let field_type = field_def
        .kind
        .as_ref()
        .map_or(TypeAST::Scalar(ScalarType::Any), |kind| {
            TypeAST::from(kind.clone())
        });

    match parts.last().unwrap() {
        surrealdb::sql::Part::All => {
            if let TypeAST::Array(obj) = &mut curr.ast {
                let ast = &mut (*obj).0;
                *ast = field_type;
            } else {
                return Err(SchemaParseError::NonArrayStarSelector(
                    parts
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join("."),
                ));
            }
        }
        surrealdb::sql::Part::Field(ident) => {
            let field_name = ident.to_string();
            current_path.push(field_name.clone());
            if let TypeAST::Object(obj) = &mut curr.ast {
                let new_field = FieldInfo {
                    ast: if field_def
                        .kind
                        .as_ref()
                        .map_or(false, |k| matches!(k, Kind::Array(_, _)))
                    {
                        TypeAST::Array(Box::new((TypeAST::Scalar(ScalarType::Any), None)))
                    } else {
                        field_type
                    },
                    meta: FieldMetadata {
                        original_name: field_name.clone(),
                        original_path: current_path,
                        permissions: field_def.permissions.clone(),
                    },
                };
                obj.fields.insert(field_name, new_field);
            }
        }
        _ => {
            return Err(SchemaParseError::Unknown(
                "Unexpected last part in field definition".to_string(),
            ))
        }
    }

    Ok(())
}

fn apply_param_definition(
    param_def: &DefineParamStatement,
    ast: &mut TypeAST,
) -> Result<(), SchemaParseError> {
    // Implement param definition logic here
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use surrealdb::sql::parse;

    #[test]
    fn test_nested_objects() {
        let schema = r#"
            DEFINE TABLE user SCHEMAFULL;
            DEFINE FIELD address ON user TYPE object;
            DEFINE FIELD address.city ON user TYPE string;
            DEFINE FIELD address.zip ON user TYPE number;
        "#;

        let query = parse(schema).unwrap();
        let ast = analyze_schema(query).unwrap();

        if let TypeAST::Object(schema) = ast {
            if let Some(user) = schema.fields.get("user") {
                if let TypeAST::Object(user_obj) = &user.ast {
                    if let Some(address) = user_obj.fields.get("address") {
                        if let TypeAST::Object(address_obj) = &address.ast {
                            assert!(address_obj.fields.contains_key("city"));
                            assert!(address_obj.fields.contains_key("zip"));
                            return;
                        }
                    }
                }
            }
        }
        panic!("Nested object structure not found in AST");
    }

    #[test]
    fn test_nested_arrays() {
        let schema = r#"
            DEFINE TABLE post SCHEMAFULL;
            DEFINE FIELD tags ON post TYPE array;
            DEFINE FIELD tags.* ON post TYPE string;
        "#;

        let query = parse(schema).unwrap();
        let ast = analyze_schema(query).unwrap();

        if let TypeAST::Object(schema) = ast {
            if let Some(post) = schema.fields.get("post") {
                if let TypeAST::Object(post_obj) = &post.ast {
                    if let Some(tags) = post_obj.fields.get("tags") {
                        if let TypeAST::Array(inner) = &tags.ast {
                            assert!(matches!(inner.0, TypeAST::Scalar(ScalarType::String)));
                            return;
                        }
                    }
                }
            }
        }
        panic!("Nested array structure not found in AST");
    }

    #[test]
    fn test_union_types() {
        let schema = r#"
            DEFINE TABLE product SCHEMAFULL;
            DEFINE FIELD price ON product TYPE number | string;
        "#;

        let query = parse(schema).unwrap();
        let ast = analyze_schema(query).unwrap();

        if let TypeAST::Object(schema) = ast {
            if let Some(product) = schema.fields.get("product") {
                if let TypeAST::Object(product_obj) = &product.ast {
                    if let Some(price) = product_obj.fields.get("price") {
                        if let TypeAST::Union(types) = &price.ast {
                            assert!(types
                                .iter()
                                .any(|t| matches!(t, TypeAST::Scalar(ScalarType::Number))));
                            assert!(types
                                .iter()
                                .any(|t| matches!(t, TypeAST::Scalar(ScalarType::String))));
                            return;
                        }
                    }
                }
            }
        }
        panic!("Union type not found in AST");
    }

    // #[test]
    // fn test_missing_parent_object() {
    //     let schema = r#"
    //         DEFINE TABLE user SCHEMAFULL;
    //         DEFINE FIELD address.city ON user TYPE string;
    //     "#;

    //     let query = parse(schema).unwrap();
    //     let result = analyze_schema(query);

    //     assert!(matches!(
    //         result,
    //         Err(SchemaParseError::MissingParentObject(_))
    //     ));
    // }

    #[test]
    fn test_non_array_star_selector() {
        let schema = r#"
            DEFINE TABLE user SCHEMAFULL;
            DEFINE FIELD name ON user TYPE string;
            DEFINE FIELD name.* ON user TYPE string;
        "#;

        let query = parse(schema).unwrap();
        let result = analyze_schema(query);
        assert!(matches!(
            result,
            Err(SchemaParseError::NonArrayStarSelector(_))
        ));
    }
}
