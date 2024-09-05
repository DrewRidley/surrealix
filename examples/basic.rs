use dotenv::dotenv;
use surrealix_macros::{build_query, query};

build_query! {
    AdultUsers,
    "SELECT * FROM user WHERE age > 18;"
}

fn main() {
    let users = AdultUsers::execute().unwrap();
}

/*
    An example of what a strongly typed query might look like.
    Super experimental so it is very subject to change.

    Lets take this example where 'ssn' and 'dob' are only accessible to the users own record.
    All other records will be covered by IAM logic.

    In this instance, lets say 'ssn' and 'dob' have shared permissions logic. It should be possible to group them
    accordingly.

    enum UserResult {
        SSNDobUser {
            ssn,
            dob,
            friends
        },
        User {

        }
    }

    query! {
        SELECT ssn, dob, ->friend->user.* as friends FROM user;
    }


*/

/*


SELECT
    name,
    age,
    math::round(balance, 2) AS rounded_balance,
    array::len(posts) AS post_count,
    (
        SELECT
            name AS friend_name,
            (SELECT title, created_at FROM post WHERE author = user.id ORDER BY created_at DESC LIMIT 1).title AS last_post_title
        FROM
            user
        WHERE
            id = $parent.id
        LIMIT 1
    ) AS friend_info,
    (SELECT math::sum(balance) FROM user WHERE id = $parent.id) AS total_friend_balances
FROM
    user
WHERE
    age > 20
    AND
    'active' IN tags
ORDER BY
    rounded_balance DESC
LIMIT 5;

*/
