#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if let Some(code) = wonrar_core::rar_worker::maybe_run() {
        std::process::exit(code);
    }
    if let Some(code) = wonrar_scan::worker::maybe_run() {
        std::process::exit(code);
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--install-shell-integration")
    {
        match wonrar_desktop::shell::install() {
            Ok(message) => println!("{message}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    wonrar_desktop::run();
}
