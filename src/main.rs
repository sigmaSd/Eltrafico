mod limit;
mod lsof;
mod parse;
mod tc;

fn main() -> tc::CatchAll<()> {
    handle_ctrlc();

    let mut map = parse::parse()?;
    let global_limit = if let Some(limit) = map.remove("global") {
        limit
    } else {
        (None, None)
    };

    limit::limit(map, global_limit.0, global_limit.1);

    Ok(())
}

fn handle_ctrlc() {
    ctrlc::set_handler(move || {
        println!("\nCleaning up..");
        tc::clean_up("ifb0", "wlp3s0").unwrap();
    })
    .expect("Error cleaning up");
}
