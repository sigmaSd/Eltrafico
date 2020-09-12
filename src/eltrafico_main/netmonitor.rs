use crate::gui::UpdateGuiMessage;
use crate::utils::check_for_dependencies;
use crate::CatchAll;
use glib::Sender;
use std::thread;
mod bandwhich;
mod nethogs;
use bandwhich::bandwhich;
use nethogs::nethogs;

pub fn netmonitor(tx: Sender<UpdateGuiMessage>) -> CatchAll<()> {
    if check_for_dependencies(&["bandwhich"]).is_ok() {
        thread::spawn(|| {
            if let Err(e) = bandwhich(tx) {
                panic!("Bandwhich error: {}", e);
            }
        });
    } else if check_for_dependencies(&["nethogs"]).is_ok() {
        thread::spawn(|| {
            if let Err(e) = nethogs(tx) {
                panic!("Nethogs error: {}", e);
            }
        });
    }
    Ok(())
}
