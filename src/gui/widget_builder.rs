use super::Message;
use crate::utils::ifstat;
use gtk::*;
use std::sync::mpsc;

pub fn create_row(name: Option<&str>, tx2: mpsc::Sender<Message>, global: bool) -> Box {
    let title = Label::new(name);
    let down = Label::new(Some("Down: "));
    let down_value = Entry::new();
    down_value.set_placeholder_text(Some("None"));
    let up = Label::new(Some("Up: "));
    let up_value = Entry::new();
    up_value.set_placeholder_text(Some("None"));

    let set_btn = Button::new_with_label("set limit");

    let d_c = down_value.clone();
    let u_c = up_value.clone();
    let name = name.unwrap_or("?").to_string();

    // send the program name and its limits to the limiter thread
    set_btn.connect_clicked(move |_btn| {
        let send_limits = || -> Option<()> {
            let down = d_c.get_text()?.to_string();
            let down = if down.is_empty() { None } else { Some(down) };
            let up = u_c.get_text()?.to_string();
            let up = if up.is_empty() { None } else { Some(up) };

            if global {
                tx2.send(Message::Global((down, up)))
                    .expect("failed to send data to the limiter thread");
            } else {
                tx2.send(Message::Program((name.clone(), (down, up))))
                    .expect("failed to send data to the limiter thread");
            }

            Some(())
        };
        // ignore getting text from Entry widget errors
        let _ = send_limits();
    });

    let hbox = Box::new(Orientation::Horizontal, 20);
    // TODO: make the label fixed size
    hbox.pack_start(&title, true, false, 10);
    hbox.add(&down);
    hbox.add(&down_value);
    hbox.add(&up);
    hbox.add(&up_value);
    hbox.add(&set_btn);

    hbox
}

pub fn create_interface_row(tx2: mpsc::Sender<Message>) -> Box {
    let label = Label::new(Some("Interface: "));
    let combobox = ComboBoxText::new();
    let interfaces = ifstat().expect("Failed to get network interfaces");

    interfaces
        .into_iter()
        .enumerate()
        .for_each(|(idx, interface)| {
            if !interface.name.starts_with("ifb") {
                combobox.insert_text(idx as i32, &interface.name);
            }
        });

    combobox.connect_changed(move |combobox| {
        let selected_interface = combobox
            .get_active_text()
            .expect("Error reading interface name")
            .to_string();
        tx2.send(Message::Interface(selected_interface))
            .expect("Error while sending interface name to limiter thread");
    });

    let interface_row = Box::new(Orientation::Horizontal, 10);
    interface_row.add(&label);
    interface_row.add(&combobox);

    interface_row
}
