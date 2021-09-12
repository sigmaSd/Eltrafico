use super::Message;
use crate::utils::ifconfig;
use glib::object::Cast;
use gtk::prelude::*;
use gtk::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

type SharedStdinHandle = Rc<RefCell<Option<std::process::ChildStdin>>>;

pub fn create_row(name: Option<&str>, stdin: SharedStdinHandle, global: bool) -> Box {
    //TODO switch to a gtk::grid
    let name = name.unwrap_or("?").to_string();
    let print_name = format!("<b>{}</b>", &name);
    let title = Label::new(None);
    title.set_markup(&print_name);

    let current_speed = Label::new(None);
    let down = Label::new(Some("Down: "));
    let down_value = Entry::new();
    down_value.set_placeholder_text(Some("None"));
    let up = Label::new(Some("Up: "));
    let up_value = Entry::new();
    up_value.set_placeholder_text(Some("None"));

    let set_btn = Button::with_label("Set");

    let d_c = down_value.clone();
    let u_c = up_value.clone();

    // send the program name and its limits to the limiter thread
    set_btn.connect_clicked(move |btn| {
        let down = d_c.text().to_string();
        let down = if down.is_empty() { None } else { Some(down) };
        let up = u_c.text().to_string();
        let up = if up.is_empty() { None } else { Some(up) };

        if global {
            writeln!(
                stdin.borrow_mut().as_mut().unwrap(),
                "{}",
                Message::Global((down, up))
            )
            .expect("Error sending Global limit to eltrafico_tc");
        } else {
            writeln!(
                stdin.borrow_mut().as_mut().unwrap(),
                "{}",
                Message::Program((name.clone(), (down, up)))
            )
            .expect("Error sending Program limit to eltrafico_tc");
        }

        // visual feedback
        btn.set_label("Ok!");
    });

    // visual feedback
    let set_btn_c = set_btn.clone();
    down_value.connect_changed(move |_| {
        set_btn_c.set_label("Set");
    });
    let set_btn_c = set_btn.clone();
    up_value.connect_changed(move |_| {
        set_btn_c.set_label("Set");
    });

    let hbox = Box::new(Orientation::Horizontal, 20);
    // TODO: make the label fixed size
    hbox.pack_start(&title, false, false, 10);

    hbox.add(&current_speed);
    hbox.add(&down);
    hbox.add(&down_value);
    hbox.add(&up);
    hbox.add(&up_value);
    hbox.add(&set_btn);

    hbox
}

pub fn update_gui_program_speed(app_box: gtk::Box, programs_speed: HashMap<String, (f32, f32)>) {
    let programs = app_box.children();
    for program in programs {
        let program: gtk::Box = program.clone().downcast().unwrap();
        let program = program.children();
        let name: gtk::Label = program[0].clone().downcast().unwrap();
        let name = name.text().to_string();
        let speed: gtk::Label = program[1].clone().downcast().unwrap();
        if programs_speed.contains_key(&name) {
            speed.set_label(&format!(
                "Down: {:.2} KB/sec Up: {:.2} KB/sec",
                programs_speed[&name].1, programs_speed[&name].0
            ));
        } else {
            // Program data wasent sent from nethogs thread
            // That means its not active network wise anymore
            // Update label as feedback
            speed.set_label("Down: 0 KB/sec Up: 0 KB/se");
        }
    }
}

pub fn update_gui_global_speed(global_bar: gtk::Box, global_speed: (f32, f32)) {
    let speed: gtk::Label = global_bar.children()[1].clone().downcast().unwrap();
    speed.set_label(&format!(
        "Down: {:.2} KB/sec Up: {:.2} KB/sec",
        global_speed.1, global_speed.0
    ));
}

pub fn create_interface_row(stdin: SharedStdinHandle) -> Box {
    let label = Label::new(Some("Interface: "));
    let combobox = ComboBoxText::new();
    let interfaces = ifconfig().expect("Failed to get network interfaces");

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
            .active_text()
            .expect("Error reading interface name")
            .to_string();
        writeln!(
            stdin.borrow_mut().as_mut().unwrap(),
            "{}",
            Message::Interface(selected_interface)
        )
        .expect("Error sending interface to eltrafico_tc");
    });

    let interface_row = Box::new(Orientation::Horizontal, 10);
    interface_row.add(&label);
    interface_row.add(&combobox);

    interface_row
}
