// mod create;
// mod delete;
// mod function;
// mod insert;
// mod relate;
mod select;
// mod update;

use crate::errors::AnalysisError;
use crate::{ast::TypeAST, errors, schema::analyze_schema};
use select::analyze_select;
use std::collections::HashMap;
use surrealdb::sql::{Query, Statement};

pub type Tables = HashMap<String, TypeAST>;

/// Analyzes the specific query, generating a corresponding AST.
///
/// The returned value contains a [TypeAST] corresponding to each statement in the query.
/// This TypeAST encompasses all transformations performed by the query on the base schema.
/// There may be gaps in the analysis, represented by [ScalarType::Any].
pub fn analyze(schema: Query, query: Query) -> Result<Vec<TypeAST>, AnalysisError> {
    let parsed = analyze_schema(schema)?;

    query
        .iter()
        .map(|q| analyze_statement(&parsed, q))
        .collect()
}

/// Computes statement transforms over a base AST.
///
/// For top level statements, 'base_type' should contain an object for each table.
/// For other statements, base_type is the type a statement is transforming.
fn analyze_statement(base_type: &TypeAST, stmt: &Statement) -> Result<TypeAST, AnalysisError> {
    match stmt {
        Statement::Select(sel_stmt) => analyze_select(base_type, sel_stmt),
        _ => todo!("Statement: {:?} is not supported", stmt),
    }
}
