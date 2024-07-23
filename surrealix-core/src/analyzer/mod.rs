mod create;
mod delete;
mod function;
mod insert;
mod relate;
mod select;
mod update;

use crate::types::{QueryType, TypedQuery};
use std::collections::HashMap;
use surrealdb::sql::Statement;

pub type Tables = HashMap<String, TypedQuery>;

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

fn analyze_query(tables: &Tables, statement: Statement) -> Option<TypedQuery> {
    match statement {
        Statement::Select(sel) => Some(select::analyze_select(tables, &sel)),
        Statement::Create(create) => Some(create::analyze_create(tables, &create)),
        Statement::Update(update) => Some(update::analyze_update(tables, &update)),
        Statement::Delete(delete) => Some(delete::analyze_delete(tables, &delete)),
        Statement::Relate(relate) => Some(relate::analyze_relate(tables, &relate)),
        Statement::Insert(insert) => Some(insert::analyze_insert(tables, &insert)),
        _ => None,
    }
}
