//! PolyMAS domain and application services. No UI or provider credentials live here.
pub const SCHEMA_VERSION: u32 = 1;
pub mod codex;
pub mod error;
pub mod process;
pub mod registry;
pub mod scheduler;
pub mod storage;
pub mod topology;
