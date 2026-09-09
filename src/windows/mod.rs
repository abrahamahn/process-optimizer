//! Native Windows adapters. No injection, driver loading, registry tweaks or service disabling.
pub mod gpu;
pub mod process;
pub mod runner;
pub mod ui;

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
