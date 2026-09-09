//! Platform-independent policy and recovery, with narrow Windows adapters.
pub mod applications;
pub mod engine;
pub mod gpu;
pub mod integrity;
pub mod journal;
pub mod manual;
pub mod model;
pub mod policy;
pub mod profiles;
pub mod reopen;
pub mod report;
#[cfg(windows)]
pub mod windows;
