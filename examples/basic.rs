use dotenv::dotenv;
use surrealix_macros::query;

fn main() {
    let results = query! {
        SELECT address.zip FROM user;
    };
}
