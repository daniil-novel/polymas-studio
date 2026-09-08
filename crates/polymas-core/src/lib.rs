//! PolyMAS domain and application services. No UI or provider credentials live here.
pub const SCHEMA_VERSION: u32 = 1;
pub mod codex;
pub mod connections;
pub mod error;
pub mod mcp_client;
pub mod messages;
pub mod policy;
pub mod process;
pub mod registry;
pub mod scheduler;
pub mod secrets;
pub mod storage;
pub mod topology;
