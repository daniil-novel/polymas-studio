#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
fn main() {
    #[cfg(windows)]
    if let Err(error) = polymas_desktop::native::run() {
        eprintln!("PolyMAS Desktop: {error}");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("PolyMAS Desktop targets Windows. Build polymas-daemon for Linux workers.");
        std::process::exit(1);
    }
}
