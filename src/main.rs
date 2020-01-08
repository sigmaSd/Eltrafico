#[cfg(not(unix))]
compile_error!("This program is unix only for now");

mod gui;
mod limit;
mod tc;
mod utils;
use utils::check_for_iproute2;

pub type CatchAll<T> = Result<T, Box<dyn std::error::Error>>;

fn main() {
    if let Err(e) = check_for_iproute2() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    };
    gui::run();
}
