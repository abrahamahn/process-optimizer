//! Native Windows adapters. No injection, driver loading, registry tweaks or service disabling.
pub mod gpu;
pub mod manual;
pub mod process;
pub mod reopen;
pub mod runner;
pub mod simple_ui;
pub mod storage;
pub mod ui;

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
