DEFINE TABLE user SCHEMAFULL;
    DEFINE FIELD name on user TYPE string;
    DEFINE FIELD age ON user TYPE int;
    DEFINE FIELD created_at ON user TYPE datetime;
    DEFINE FIELD balance on user TYPE decimal;
    DEFINE FIELD height on user TYPE float;
    DEFINE FIELD profile_picture ON user TYPE bytes;
    DEFINE FIELD address on user TYPE object;
        DEFINE FIELD address.street on USER type string;
        DEFINE FIELD address.zip ON USER type int;
        DEFINE FIELD address.city ON USER type string;
        DEFINE FIELD address.state ON USER type string;
    DEFINE FIELD tags ON user TYPE array;
        DEFINE FIELD tags.* on user TYPE string;

DEFINE TABLE friend SCHEMAFULL;
    DEFINE FIELD in ON friend;
    DEFINE FIELD out on friend;
