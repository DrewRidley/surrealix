use std::collections::{HashMap, HashSet};

use crate::ast::{ScalarType, TypeAST};

pub fn generate_rust_types(ast: &TypeAST, externalized_paths: &HashMap<String, String>) -> String {
    let mut type_definitions = String::new();
    let mut generated_types = HashSet::new();

    generate_types_recursive(
        ast,
        externalized_paths,
        &mut type_definitions,
        &mut generated_types,
        "",
        true,
    );

    type_definitions
}

fn generate_types_recursive(
    ast: &TypeAST,
    externalized_paths: &HashMap<String, String>,
    type_definitions: &mut String,
    generated_types: &mut HashSet<String>,
    current_path: &str,
    is_public: bool,
) {
    match ast {
        TypeAST::Object(obj) => {
            let type_name = if current_path.is_empty() {
                "QueryResult".to_string()
            } else {
                format!("{}Object", current_path.replace(".", "_"))
            };

            if !generated_types.contains(&type_name) {
                generated_types.insert(type_name.clone());

                let visibility = if is_public { "pub " } else { "" };
                type_definitions.push_str(&format!("\n{}struct {} {{\n", visibility, type_name));

                for (field_name, field_info) in &obj.fields {
                    let field_path = if current_path.is_empty() {
                        field_name.clone()
                    } else {
                        format!("{}.{}", current_path, field_name)
                    };

                    let (field_type, is_option) = get_rust_type(&field_info.ast);
                    let field_visibility = if externalized_paths.contains_key(&field_path) {
                        "pub "
                    } else {
                        ""
                    };

                    type_definitions.push_str(&format!(
                        "    {}{}: {}{},\n",
                        field_visibility,
                        field_name,
                        if is_option { "Option<" } else { "" },
                        field_type
                    ));

                    if is_option {
                        type_definitions.push('>');
                    }

                    generate_types_recursive(
                        &field_info.ast,
                        externalized_paths,
                        type_definitions,
                        generated_types,
                        &field_path,
                        false,
                    );
                }

                type_definitions.push_str("}\n");
            }
        }
        TypeAST::Array(inner) => {
            generate_types_recursive(
                &inner.0,
                externalized_paths,
                type_definitions,
                generated_types,
                current_path,
                false,
            );
        }
        TypeAST::Option(inner) => {
            generate_types_recursive(
                inner,
                externalized_paths,
                type_definitions,
                generated_types,
                current_path,
                false,
            );
        }
        _ => {}
    }
}

fn get_rust_type(ast: &TypeAST) -> (String, bool) {
    match ast {
        TypeAST::Scalar(scalar_type) => (scalar_type_to_rust_type(scalar_type), false),
        TypeAST::Object(obj) => {
            let type_name = format!("{}Object", obj.fields.keys().next().unwrap());
            (type_name, false)
        }
        TypeAST::Array(inner) => {
            let (inner_type, _) = get_rust_type(&inner.0);
            (format!("Vec<{}>", inner_type), false)
        }
        TypeAST::Option(inner) => {
            let (inner_type, _) = get_rust_type(inner);
            (inner_type, true)
        }
        TypeAST::Record(table) => (format!("Record<{}>", table), false),
        TypeAST::Union(_) => ("serde_json::Value".to_string(), false),
    }
}

fn scalar_type_to_rust_type(scalar_type: &ScalarType) -> String {
    match scalar_type {
        ScalarType::String => "String".to_string(),
        ScalarType::Integer => "i64".to_string(),
        ScalarType::Number => "f64".to_string(),
        ScalarType::Float => "f32".to_string(),
        ScalarType::Boolean => "bool".to_string(),
        ScalarType::Point => "Point".to_string(),
        ScalarType::Geometry => "Geometry".to_string(),
        ScalarType::Set => "Set".to_string(),
        ScalarType::Datetime => "DateTime<Utc>".to_string(),
        ScalarType::Duration => "Duration".to_string(),
        ScalarType::Bytes => "Vec<u8>".to_string(),
        ScalarType::Uuid => "Uuid".to_string(),
        ScalarType::Any => "serde_json::Value".to_string(),
        ScalarType::Null => "()".to_string(),
    }
}
