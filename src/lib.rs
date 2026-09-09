//! Platform-independent policy and recovery, with narrow Windows adapters.
pub mod engine;
pub mod gpu;
pub mod integrity;
pub mod journal;
pub mod model;
pub mod policy;
#[cfg(windows)]
pub mod windows;
