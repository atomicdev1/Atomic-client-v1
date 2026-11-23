mod cli;
mod engine;

#[cfg(feature = "tauri")]
mod commands;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "cli" {
        cli::run(&args[2..]);
    } else {
        #[cfg(feature = "tauri")]
        {
            atomic_client_lib::run_tauri();
        }
        #[cfg(not(feature = "tauri"))]
        {
            eprintln!("Tauri desktop feature not enabled.");
            eprintln!("Build with: cargo build --release --features tauri");
            eprintln!("Or use CLI: atomic-client cli <command> [args]");
            std::process::exit(1);
        }
    }
}
