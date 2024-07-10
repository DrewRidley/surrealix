use crate::{FieldDefinition, FieldType, TableDefinition};
use std::collections::HashMap;
use surrealdb::sql::Kind;

#[derive(Debug, Clone)]
pub enum QueryType {
    Object(HashMap<String, QueryType>),
    Array(Box<QueryType>),
    Value(Kind),
}

impl QueryType {
    pub fn from_table(table: &TableDefinition) -> Self {
        println!("Creating QueryType from table: {}", table.name);

        let mut fields = HashMap::new();
        for field in &table.fields {
            fields.insert(field.name.clone(), QueryType::from_field(field));
        }
        QueryType::Object(fields)
    }

    pub fn from_field(field: &FieldDefinition) -> Self {
        println!("Creating QueryType from field: {}", field.name);

        match &field.field_type {
            FieldType::Inline(kind) => QueryType::Value(kind.clone()),
            FieldType::Array((inner_field, _)) => {
                QueryType::Array(Box::new(QueryType::from_field(inner_field)))
            }
            FieldType::Object(object_fields) => {
                let mut fields = HashMap::new();
                for object_field in object_fields {
                    fields.insert(
                        object_field.name.clone(),
                        QueryType::from_field(object_field),
                    );
                }
                QueryType::Object(fields)
            }
        }
    }
}
