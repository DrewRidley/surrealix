use std::fmt;
use std::{collections::HashMap, num::NonZeroU64};
use surrealdb::sql::{Fields, Idiom, Kind, Part, Permissions, Value};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AstError {
    #[error("Unknown field: {0}")]
    UnknownField(String),
    #[error("Invalid field type")]
    InvalidFieldType,
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
}

#[derive(Clone, PartialEq, Eq)]
pub enum TypeAST {
    Scalar(ScalarType),
    Object(ObjectType),
    Array(Box<(TypeAST, Option<NonZeroU64>)>),
    Option(Box<TypeAST>),
    Record(String),
    Union(Vec<TypeAST>),
}

impl TypeAST {
    pub fn resolve_fields(&self, fields: &Fields) -> Result<TypeAST, AstError> {
        match self {
            TypeAST::Object(obj) => {
                let mut result = ObjectType {
                    fields: HashMap::new(),
                };
                for field in &fields.0 {
                    match field {
                        surrealdb::sql::Field::All => {
                            result.fields = obj.fields.clone();
                            break;
                        }
                        surrealdb::sql::Field::Single { expr, alias } => {
                            if let Value::Idiom(idiom) = expr {
                                let field_name = idiom.to_string();
                                if let Some(field_info) = obj.fields.get(&field_name) {
                                    let result_name =
                                        alias.as_ref().map(|a| a.to_string()).unwrap_or(field_name);
                                    result.fields.insert(result_name, field_info.clone());
                                } else {
                                    return Err(AstError::UnknownField(field_name));
                                }
                            }
                        }
                    }
                }
                Ok(TypeAST::Object(result))
            }
            _ => Err(AstError::InvalidFieldType),
        }
    }

    pub fn resolve_idiom(&self, idiom: &Idiom) -> Result<&TypeAST, AstError> {
        let mut current = self;
        for part in &idiom.0 {
            match (current, part) {
                (TypeAST::Object(obj), Part::Field(ident)) => {
                    let field_name = ident.to_string();
                    if let Some(field_info) = obj.fields.get(&field_name) {
                        current = &field_info.ast;
                    } else {
                        return Err(AstError::UnknownField(field_name));
                    }
                }
                (TypeAST::Array(boxed), Part::All) => {
                    current = &boxed.0;
                }
                _ => return Err(AstError::InvalidFieldType),
            }
        }
        Ok(current)
    }

    pub fn replace_record_links(&mut self, schema: &TypeAST) -> Result<(), AstError> {
        match self {
            TypeAST::Object(obj) => {
                for field_info in obj.fields.values_mut() {
                    field_info.ast.replace_record_links(schema)?;
                }
            }
            TypeAST::Array(boxed) => {
                boxed.0.replace_record_links(schema)?;
            }
            TypeAST::Record(table_name) => {
                if let TypeAST::Object(schema_obj) = schema {
                    if let Some(table_ast) = schema_obj.fields.get(table_name) {
                        *self = table_ast.ast.clone();
                    } else {
                        return Err(AstError::UnknownField(table_name.clone()));
                    }
                }
            }
            TypeAST::Union(variants) => {
                for variant in variants {
                    variant.replace_record_links(schema)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl From<Kind> for TypeAST {
    fn from(value: Kind) -> Self {
        match value {
            Kind::Object => TypeAST::Object(ObjectType::default()),
            Kind::Record(rec) => TypeAST::Record(rec.first().unwrap().to_string()),
            Kind::Option(inner_kind) => TypeAST::Option(Box::new(TypeAST::from(*inner_kind))),
            Kind::Set(kind, len) | Kind::Array(kind, len) => TypeAST::Array(Box::new((
                TypeAST::from(*kind),
                len.map(|v| NonZeroU64::new(v).expect("array length is not zero.")),
            ))),
            Kind::Either(kind) => TypeAST::Union(kind.into_iter().map(TypeAST::from).collect()),
            kind => TypeAST::Scalar(ScalarType::from(kind)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarType {
    String,
    Integer,
    Number,
    Float,
    Boolean,
    Point,
    Geometry,
    Set,
    Datetime,
    Duration,
    Bytes,
    Uuid,
    Any,
    Null,
}

impl From<Kind> for ScalarType {
    fn from(value: Kind) -> Self {
        match value {
            Kind::Any => Self::Any,
            Kind::Null => Self::Null,
            Kind::Bool => Self::Boolean,
            Kind::Bytes => Self::Bytes,
            Kind::Datetime => Self::Datetime,
            Kind::Decimal => Self::Number,
            Kind::Duration => Self::Duration,
            Kind::Float => Self::Float,
            Kind::Int => Self::Integer,
            Kind::Number => Self::Number,
            Kind::String => Self::String,
            Kind::Uuid => Self::Uuid,
            Kind::Point => Self::Point,
            Kind::Geometry(_) => ScalarType::Geometry,
            _ => panic!("Cannot convert complex Kind to ScalarType"),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct ObjectType {
    pub fields: HashMap<String, FieldInfo>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FieldInfo {
    pub ast: TypeAST,
    pub meta: FieldMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMetadata {
    pub original_name: String,
    pub original_path: Vec<String>,
    pub permissions: Permissions,
}

impl TypeAST {
    fn fmt_with_indent(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        let indent_str = "  ".repeat(indent);
        match self {
            TypeAST::Scalar(scalar) => write!(f, "{:?}", scalar),
            TypeAST::Object(obj) => {
                writeln!(f, "{{")?;
                for (name, field) in &obj.fields {
                    write!(f, "{}  {}", indent_str, name)?;
                    if matches!(field.ast, TypeAST::Option(_)) {
                        write!(f, "?: ")?;
                    } else {
                        write!(f, ": ")?;
                    }
                    match &field.ast {
                        TypeAST::Option(inner) => inner.fmt_with_indent(f, indent + 1)?,
                        _ => field.ast.fmt_with_indent(f, indent + 1)?,
                    }
                    writeln!(f, ",")?;
                }
                write!(f, "{}}}", indent_str)
            }
            TypeAST::Array(inner) => {
                write!(f, "[")?;
                inner.0.fmt_with_indent(f, indent)?;
                if let Some(len) = inner.1 {
                    write!(f, "; {}]", len)
                } else {
                    write!(f, "]")
                }
            }
            TypeAST::Option(inner) => inner.fmt_with_indent(f, indent),
            TypeAST::Record(table) => write!(f, "Record({})", table),
            TypeAST::Union(variants) => {
                write!(f, "Union(")?;
                for (i, variant) in variants.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    variant.fmt_with_indent(f, indent)?;
                }
                write!(f, ")")
            }
        }
    }
}

impl fmt::Debug for TypeAST {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_indent(f, 0)
    }
}

impl fmt::Debug for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectType")
            .field("fields", &self.fields)
            .finish()
    }
}

impl fmt::Debug for FieldInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldInfo")
            .field("ast", &self.ast)
            .field("meta", &self.meta)
            .finish()
    }
}
