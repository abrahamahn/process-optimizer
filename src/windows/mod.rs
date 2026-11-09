//! Native Windows adapters. No injection, driver loading, service disabling, or undocumented registry tweaks.
pub mod backend;
pub mod gpu;
pub mod manual;
pub mod process;
pub mod reopen;
pub mod runner;
mod simple_recommendations;
pub mod simple_ui;
pub mod startup;
pub mod storage;
pub mod ui;
mod ui_common;
pub mod update;

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
