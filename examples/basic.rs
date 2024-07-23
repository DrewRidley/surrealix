use dotenv::dotenv;
use surrealix_macros::query;

fn main() {
    let myUser;

    let results = query! {
       r#"
            SELECT * FROM user FETCH posts;
       "#
    };
}

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
