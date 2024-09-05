pub use surrealix_macros::query;

pub mod types {
    pub use surrealix_core::{DateTime, Duration, RecordLink};
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct RecordLink(pub String);
