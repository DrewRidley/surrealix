use std::collections::HashMap;

use select::analyze_select;

use crate::types::{QueryType, TypedQuery};

mod select;

/// Analyze a single independent query statement.
/// 'SELECT', 'UPDATE', 'CREATE', 'UPSERT' and 'DELETE' are all valid in this context.
/// 'LIVE' is not generally supported because it doesn't need type analysis.
fn analyze_query(tables: &Tables, statement: surrealdb::sql::Statement) -> Option<TypedQuery> {
    match statement {
        surrealdb::sql::Statement::Select(sel) => Some(analyze_select(&tables, sel)),
        surrealdb::sql::Statement::Remove(_) => todo!(),
        surrealdb::sql::Statement::Create(_) => todo!(),
        surrealdb::sql::Statement::Delete(_) => todo!(),
        surrealdb::sql::Statement::Insert(_) => todo!(),
        surrealdb::sql::Statement::Live(_) => {
            panic!("analysis of LIVE statements is not supported!");
        }
        surrealdb::sql::Statement::Relate(_) => todo!("Relations are not supported!"),
        _ => None,
    }
}

pub(crate) type Tables = HashMap<String, TypedQuery>;

/// Analyzes a complete query string and returns the aliased or mutated type info.
///
/// Accepts 'definitions', the type definitions generated from the schema
/// As well as 'query' which contains the full query to be analyzed in question.
pub fn analyze(tables: Tables, query: String) -> Vec<TypedQuery> {
    println!("Query string: [{:?}]", query);
    let parsed = surrealdb::sql::parse(query.as_str()).unwrap();

    let mut data = vec![];
    for statement in parsed {
        if let Some(val) = analyze_query(&tables, statement) {
            data.push(val);
        }
    }
    data
}
