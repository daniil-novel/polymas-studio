//! PolyMAS domain and application services. No UI or provider credentials live here.
pub const SCHEMA_VERSION: u32 = 1;
pub mod error;
pub mod registry;
pub mod storage;
pub mod topology;
