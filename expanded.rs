#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use dotenv::dotenv;
use surrealix_macros::build_query;
pub struct AdultUsers;
impl AdultUsers {
    pub fn execute() -> Result<adult_users::QueryResult, surrealix::Error> {
        {
            ::core::panicking::panic_fmt(format_args!(
                "not yet implemented: {0}",
                format_args!("Implement execute method"),
            ));
        }
    }
}
pub mod adult_users {
    use super::*;
    pub struct UserAddress {
        pub city: String,
        pub street: String,
        pub state: String,
        pub zip: i64,
    }

    pub struct User {
        pub balance: f64,
        pub height: f32,
        pub age: i64,
        pub address: UserAddress,
        pub tags: Vec<String>,
        pub posts: Vec<RecordLink<Post>>,
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub name: String,
        pub profile_picture: Vec<u8>,
        pub ssn: String,
    }

    pub type QueryResult = Vec<User>;
}
fn main() {
    let data = AdultUsers::execute().unwrap();
    for entry in data {
        let name = entry.name;
        let age = entry.age;
    }
}
