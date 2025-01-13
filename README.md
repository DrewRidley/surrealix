# Surrealix

Surrealix is a type-safe query builder for SurrealDB that brings compile-time query validation and type inference to your Rust projects. It analyzes your database schema and queries at compile time to ensure type safety and provide rich IDE support.

[![Crates.io](https://img.shields.io/crates/v/surrealix.svg)](https://crates.io/crates/surrealix)
[![Documentation](https://docs.rs/surrealix/badge.svg)](https://docs.rs/surrealix)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Features

- 🔍 **Compile-time Query Validation**: Catch query errors before runtime
- 🎯 **Type Inference**: Automatically derive Rust types from your SurrealDB schema
- 🚀 **Zero Runtime Overhead**: All analysis happens at compile time
- 💡 **Rich IDE Support**: Get autocompletion and type hints for your queries
- 🔒 **Type-safe Record Links**: Properly typed relationships between tables
- 🌐 **Framework Integration**: (Coming Soon)
  - React/Next.js bindings
  - SvelteKit bindings
  - More frameworks planned

## Installation

Add Surrealix to your `Cargo.toml`:

```toml
[dependencies]
surrealix = "0.1.0"
```

## Quick Start

1. Define your schema:

```sql
DEFINE TABLE user SCHEMAFULL;
    DEFINE FIELD name ON user TYPE string;
    DEFINE FIELD age ON user TYPE number;
    DEFINE FIELD tags ON user TYPE array;
        DEFINE FIELD tags.* ON user TYPE string;
```

2. Create a `.env` file in your project root:

```env
SURREALIX_SCHEMA_PATH=./schema.surql
```

3. Use the query builder:

```rust
use surrealix_macros::build_query;

build_query! {
    AdultUsers,
    "SELECT name FROM user WHERE age > 18;"
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adults = AdultUsers::execute()?;
    for user in adults {
        println!("Adult user: {}", user.name);
    }
    Ok(())
}
```

## How It Works

Surrealix works by:
1. Reading your SurrealDB schema at compile time
2. Analyzing your queries for correctness
3. Generating type-safe Rust code
4. Providing a runtime that efficiently executes the queries

## Advanced Usage

### Record Links

```rust
build_query! {
    UserWithFriends,
    "SELECT *, ->friend->user.name as friend_names FROM user;"
}
```

### Complex Types

```rust
build_query! {
    UserProfile,
    "SELECT
        name,
        tags,
        ->posted->article.* as articles
    FROM user
    FETCH articles;"
}
```

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Prerequisites

- Rust (latest stable)
- SurrealDB
- Node.js (for web framework integrations)

### Local Development

```bash
# Clone the repository
git clone https://github.com/yourusername/surrealix.git
cd surrealix

# Run tests
cargo test

# Run examples
cargo run --example basic
```

## Roadmap

- [ ] Complete Web Framework Integrations
- [ ] Query Parameter Support
- [ ] Transaction Support
- [ ] Migration Tools
- [ ] CLI Tools
- [ ] Query Performance Optimization

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- SurrealDB team for creating an amazing database
- Rust community for inspiration and support

## Support

- [Issues](https://github.com/yourusername/surrealix/issues)
- [Discussions](https://github.com/yourusername/surrealix/discussions)
- [Discord](your-discord-link)
```
