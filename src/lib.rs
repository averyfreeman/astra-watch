//! Astra Watch adds project contracts to Codex; it does not replace its model runtime.
pub mod bootstrap;
pub mod cadence;
pub mod config;
pub mod dashboard;
pub mod files;
pub mod integration;
pub mod knowledge;
pub mod mcp;
pub mod operation;
pub mod store;
pub mod tasks;
pub mod telemetry;
pub mod verify;

use sha2::{Digest, Sha256};

pub fn digest(bytes: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(bytes.as_ref()))
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
