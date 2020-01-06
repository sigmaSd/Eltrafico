use crate::limit;
use crate::Rate;
use gio::prelude::*;
use gtk::*;
use std::env::args;
use std::sync::mpsc;
use std::thread;

fn build_ui(application: &gtk::Application) {
    // channels
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let (tx2, rx2) = mpsc::channel();
    let tx2_c = tx2.clone();

    // ui build
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("ElTrafico");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(300, 500);
    window.connect_delete_event(move |_, _| {
        while let Ok(_) = tx2_c.send(("stop".to_string(), (None, None))) {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        Inhibit(false)
    });

    let main_box = Box::new(Orientation::Vertical, 10);
    let interface_row = create_interface_row(tx2.clone());
    let global_bar = create_row(Some("global"), tx2.clone());
    let app_box = Box::new(Orientation::Vertical, 10);
    let app_box_c = app_box.clone();

    main_box.add(&interface_row);
    main_box.add(&global_bar);
    main_box.add(&app_box_c);
    window.add(&main_box);
    window.show_all();

    // spawn limiter thread
    thread::spawn(|| {
        if let Err(e) = limit::limit(Some(2), tx, rx2) {
            panic!("Something happened: {}", e);
        }
    });

    // callback to add new programs to gui
    rx.attach(None, move |program| {
        let app_bar = create_row(Some(&program), tx2.clone());
        app_box.add(&app_bar);
        app_box.show_all();
        glib::Continue(true)
    });
}

pub fn run() {
    let application = gtk::Application::new(Some("com.github.eltrfico"), Default::default())
        .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&args().collect::<Vec<_>>());
}

fn create_row(name: Option<&str>, tx2: mpsc::Sender<(String, (Rate, Rate))>) -> Box {
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
            tx2.send((name.clone(), (down, up)))
                .expect("failed to send data to the limiter thread");
            Some(())
        };
        // ignore getting text from Entry widget errors
        let _ = send_limits();
    });

    let hbox = Box::new(Orientation::Horizontal, 20);
    hbox.add(&title);
    hbox.add(&down);
    hbox.add(&down_value);
    hbox.add(&up);
    hbox.add(&up_value);
    hbox.add(&set_btn);

    hbox
}

fn create_interface_row(tx2: mpsc::Sender<(String, (Rate, Rate))>) -> Box {
    let label = Label::new(Some("Interface: "));
    let combobox = ComboBoxText::new();
    let interfaces = crate::utils::ifconfig().expect("Failed to get network interfaces");

    interfaces
        .into_iter()
        .enumerate()
        .for_each(|(idx, interface)| {
            if interface.is_up() && !interface.name.starts_with("ifb") {
                combobox.insert_text(idx as i32, &interface.name);
            }
        });

    combobox.connect_changed(move |combobox| {
        // jumping on the limit channel
        // so we dont create a new channel
        // it it cool?
        let selected_interface = combobox.get_active_text().map(|s| s.to_string());
        tx2.send((
            "interface".to_string(),
            (selected_interface.clone(), selected_interface),
        ))
        .expect("Error while sending interface name to limiter thread");
    });

    let interface_row = Box::new(Orientation::Horizontal, 10);
    interface_row.add(&label);
    interface_row.add(&combobox);

    interface_row
}
