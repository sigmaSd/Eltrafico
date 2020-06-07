#[cfg(not(unix))]
compile_error!("This program is unix only for now");

mod gui;
mod limit;
mod tc;
mod utils;
use utils::{check_for_dependencies, is_root};
mod nethogs;

pub type CatchAll<T> = Result<T, Box<dyn std::error::Error>>;
const DEPENDENCIES: [&str; 4] = ["tc", "ss", "ifconfig", "ip"];

fn main() {
    if !is_root().expect("Error while verifying root permission") {
        eprintln!("This program needs sudo privilege");
        std::process::exit(1);
    }
    if let Err(e) = check_for_dependencies(&DEPENDENCIES) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    };
    gui::run();
}
