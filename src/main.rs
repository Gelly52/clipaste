mod common;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("clipaste {}", common::VERSION);
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        common::print_help();
        return;
    }

    #[cfg(target_os = "macos")]
    macos::run();

    #[cfg(target_os = "windows")]
    windows::run();

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        eprintln!("clipaste: unsupported platform");
        std::process::exit(1);
    }
}
