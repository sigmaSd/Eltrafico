mod widget_builder;
use crate::limit;
use gio::prelude::*;
use gtk::*;
use std::env::args;
use std::sync::mpsc;
use std::thread;
use widget_builder::{create_interface_row, create_row};

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
        while let Ok(_) = tx2_c.send(Message::Stop) {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        Inhibit(false)
    });

    let main_box = Box::new(Orientation::Vertical, 10);
    let interface_row = create_interface_row(tx2.clone());
    let global_bar = create_row(Some("global"), tx2.clone(), true);
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
        let app_bar = create_row(Some(&program), tx2.clone(), false);
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

#[derive(PartialEq)]
pub enum Message {
    Stop,
    Interface(String),
    Global((Option<String>, Option<String>)),
    Program((String, (Option<String>, Option<String>))),
}
