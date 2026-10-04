//! Intentional process diagnostics. No host paths, environment, or document data.
use serde::Serialize;

#[derive(Serialize)]
pub struct IndexStatus {
    pub name: String,
    pub generation: Option<String>,
    pub state: &'static str,
    pub persisted: Option<bool>,
    pub schema_name: Option<String>,
    pub warning: Option<&'static str>,
}

#[derive(Serialize)]
pub struct DaemonStatus {
    pub version: &'static str,
    pub ready: bool,
    pub native_runtime: &'static str,
    pub lifecycle: &'static str,
    pub indexes: Vec<IndexStatus>,
    pub unavailable: [&'static str; 4],
}
