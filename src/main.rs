mod limit;
mod lsof;
mod parse;
mod tc;
mod utils;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
pub type CatchAll<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> CatchAll<()> {
    // flag to stop the prgoram when done (with ctrlc)
    let running = Arc::new(AtomicBool::new(true));
    let mut args = std::env::args().skip(1);
    let interface = args.next().expect("No interface specified");

    let config = args.next().expect("No config specified");
    let delay = args
        .next()
        .map(|d| d.parse().expect("Error parsing delay duration"));

    let mut map = parse::parse(config).expect("Error while parsing config file");

    let global_limit = if let Some(limit) = map.remove("global") {
        limit
    } else {
        (None, None)
    };

    handle_ctrlc(running.clone());

    if let Err(e) = limit::limit(
        map,
        global_limit.0,
        global_limit.1,
        &interface,
        delay,
        running,
    ) {
        eprintln!("Something happened: {}", e);
    }

    clean_up(&interface).expect("Error while cleaning up");

    Ok(())
}

fn clean_up(interface: &str) -> CatchAll<()> {
    println!("\nCleaning up..");
    tc::clean_up(&tc::acquire_ifb_device()?, &interface)?;
    Ok(())
}

fn handle_ctrlc(running: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        running.store(false, Ordering::SeqCst);
    })
    .expect("Error handling ctrlc signal");
}
