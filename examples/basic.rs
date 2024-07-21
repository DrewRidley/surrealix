use dotenv::dotenv;
use surrealix_macros::query;

fn main() {
    let myUser;

    let results = query! {
       SELECT name, age, address as addy, ->friend->user.* as friends FROM user;
    };
}
