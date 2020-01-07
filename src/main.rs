mod gui;
mod limit;
mod lsof;
mod tc;
mod utils;
pub type CatchAll<T> = Result<T, Box<dyn std::error::Error>>;

fn main() {
    gui::run();
}
