use thiserror::Error;

use crate::{ast::ResolverError, schema::SchemaParseError};

#[derive(Error, Debug)]
pub enum SchemaError {
    /// In order to do type analysis, Surrealix needs to read the schema.
    /// For now, the easiest way to allow configuration of this is through a '.env' file.
    /// In most projects, this '.env' should be committed to the repo.
    #[error(
        "Environment variable not set: {0}.
        Please create a '.env' file and set this variable.
        Refer to documentation for more details."
    )]
    EnvVarNotSet(String),

    /// The schema file could not be read.
    #[error("Failed to read schema file: {0}")]
    FileReadError(std::io::Error),

    /// The 'local database' option was used, but there was an error updating the schema.
    #[error("Database connection error: {0}")]
    DatabaseConnectionError(#[from] surrealdb::Error),

    #[error("Failed to parse schema file as valid SurrealQL: {0}")]
    SchemaParseError(surrealdb::Error),

    #[error("Failed to load .env file: {0}")]
    DotEnvError(#[from] dotenv::Error),
}

#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Statement references an unknown field: {0}")]
    UnknownField(String),
    #[error("Statement uses a type that is not currently supported: {0}")]
    UnsupportedType(String),
    #[error("Statement performs an operation that is not supported: {0}")]
    UnsupportedOperation(String),
    #[error("Failure resolving a path in the schema: {0}")]
    ResolverFailure(#[from] ResolverError),

    #[error(transparent)]
    SchemaParseError(#[from] SchemaParseError),
}
