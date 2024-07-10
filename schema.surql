DEFINE TABLE user SCHEMAFULL;

DEFINE FIELD name ON user TYPE string;
DEFINE FIELD age ON user TYPE int;
DEFINE FIELD created_at ON user TYPE datetime;
DEFINE FIELD id ON user TYPE uuid;
DEFINE FIELD balance ON user TYPE decimal;
DEFINE FIELD height ON user TYPE float;
DEFINE FIELD session_duration ON user TYPE duration;
DEFINE FIELD profile_picture ON user TYPE bytes;

DEFINE FIELD address ON user TYPE object;
DEFINE FIELD address.street on USER type string;
DEFINE FIELD address.zip on USER type int;

DEFINE FIELD tags ON user TYPE array;
DEFINE FIELD tags.* ON user TYPE string;

DEFINE FIELD fixed_tags ON user TYPE array;


DEFINE FIELD status ON user TYPE option<string>;

DEFINE FIELD account_type ON user TYPE string ASSERT $value INSIDE ['personal', 'business'];

DEFINE FIELD location ON user TYPE geometry(point);

DEFINE FIELD friends ON user TYPE array;
DEFINE FIELD friends.* ON user TYPE string;

DEFINE FIELD favorite_numbers ON user TYPE array;
DEFINE FIELD favorite_numbers.* ON user TYPE int;

DEFINE FIELD email ON user TYPE string ASSERT string::is::email($value);

DEFINE FIELD country_code ON user TYPE string ASSERT $value = /^[A-Z]{3}$/;

DEFINE TABLE post SCHEMAFULL;

DEFINE FIELD title ON post TYPE string;
DEFINE FIELD content ON post TYPE string;
DEFINE FIELD tags ON post TYPE array;
DEFINE FIELD tags.* ON post TYPE string;
DEFINE FIELD published_at ON post TYPE datetime;

DEFINE FIELD test ON post TYPE object;
DEFINE FIELD test.a ON post TYPE string;
DEFINE FIELD test.b ON post TYPE number;

DEFINE TABLE comment SCHEMAFULL;

DEFINE FIELD content ON comment TYPE string;
DEFINE FIELD created_at ON comment TYPE datetime;
DEFINE FIELD likes ON comment TYPE int;

DEFINE TABLE product SCHEMAFULL;

DEFINE FIELD name ON product TYPE string;
DEFINE FIELD price ON product TYPE decimal;
DEFINE FIELD category ON product TYPE string;
DEFINE FIELD in_stock ON product TYPE bool;
DEFINE FIELD dimensions ON product TYPE object;
