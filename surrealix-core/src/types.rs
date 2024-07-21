use std::{
    collections::HashMap,
    num::{NonZeroU16, NonZeroU64},
};
use surrealdb::sql::{Kind, Permissions};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedQuery {
    pub query_type: QueryType,
    pub perms: Permissions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryType {
    /// A simple value.
    Scalar(Kind),
    /// A nested object
    Object(HashMap<String, TypedQuery>),
    /// A SurrealQL array.
    /// The second value is the optional size limit.
    Array(Option<Box<TypedQuery>>, Option<NonZeroU64>),
    /// A record link, with the table name specified.
    Record(String),
    /// An optional type that may be nullable as specified by the schema.
    Option(Box<TypedQuery>),
}
