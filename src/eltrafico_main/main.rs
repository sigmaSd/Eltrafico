#[cfg(not(unix))]
compile_error!("This program is unix only for now");

mod gui;
mod utils;
use utils::check_for_dependencies;
mod nethogs;

pub type CatchAll<T> = Result<T, Box<dyn std::error::Error>>;
const DEPENDENCIES: [&str; 4] = ["tc", "ss", "ifconfig", "ip"];

fn main() {
    if let Err(e) = check_for_dependencies(&DEPENDENCIES) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    };
    gui::run();
}
