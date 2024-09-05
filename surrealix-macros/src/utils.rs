
fn load_env() -> Result<(), SchemaError> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| SchemaError::EnvVarNotSet("CARGO_MANIFEST_DIR".to_string()))?;
    let mut env_path = PathBuf::from(manifest_dir);
    env_path.push(".env");

    dotenv::from_path(env_path)?;
    Ok(())
}

fn fetch_schema() -> Result<String, SchemaError> {
    load_env()?;

    // Fallback to schema file in debug mode, or primary method in release mode
    let path = env::var("SURREALIX_SCHEMA_PATH")
        .map_err(|_| SchemaError::EnvVarNotSet("SURREALIX_SCHEMA_PATH".to_string()))?;

    let path = if path.starts_with("./") || !path.starts_with('/') {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR")
            .map_err(|_| SchemaError::EnvVarNotSet("CARGO_MANIFEST_DIR".to_string()))?;
        let mut path_buf = PathBuf::from(manifest_dir);
        path_buf.push(path.trim_start_matches("./"));
        path_buf
    } else {
        PathBuf::from(path)
    };

    fs::read_to_string(path).map_err(SchemaError::FileReadError)
}
