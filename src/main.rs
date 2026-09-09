#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use process_optimizer::windows::{runner, ui};
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("--session") => args.get(2).ok_or_else(|| "Missing session ID.".to_string()).and_then(|id| runner::run(id)),
        Some("--recover") => runner::recover(),
        Some("--acknowledge") => runner::acknowledge(),
        Some("--probe") => args.get(2).ok_or_else(|| "Missing output path.".to_string()).and_then(|p| runner::write_probe(std::path::Path::new(p))),
        Some("--ui-smoke") => ui::smoke(),
        None => ui::run(),
        _ => Err("Unknown option. Open the application without arguments.".into()),
    };
    if let Err(error) = result {
        if matches!(args.get(1).map(String::as_str), Some("--probe" | "--ui-smoke")) {
            eprintln!("{error}");
        } else { ui::error(&error); }
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() { eprintln!("The native application runs on Windows. Platform-independent tests can run here with cargo test --lib."); }
