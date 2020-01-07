#[cfg(not(unix))]
compile_error!("This program is unix only for now");

mod gui;
mod limit;
mod tc;
mod utils;
pub type CatchAll<T> = Result<T, Box<dyn std::error::Error>>;

fn main() {
    gui::run();
}
