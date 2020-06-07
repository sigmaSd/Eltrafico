mod widget_builder;
use crate::limit;
use crate::nethogs::nethogs;
use crate::utils::check_for_dependencies;
use gio::prelude::*;
use gtk::*;
use std::collections::HashMap;
use std::env::args;
use std::sync::mpsc;
use std::thread;
use widget_builder::*;

fn build_ui(application: &gtk::Application) {
    // channels
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let tx_c = tx.clone();

    let (tx2, rx2) = mpsc::channel();
    let tx2_c = tx2.clone();
    let tx2_cc = tx2.clone();

    // ui build
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("ElTrafico");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(300, 500);

    // Cleanup at exit
    window.connect_delete_event(move |_, _| {
        tx2_c.send(Message::Stop).expect("error sending stop msg");
        Inhibit(true)
    });
    glib::source::unix_signal_add(2, move || {
        tx2_cc.send(Message::Stop).expect("error sending stop msg");
        Continue(false)
    });

    let main_box = Box::new(Orientation::Vertical, 10);
    let interface_row = create_interface_row(tx2.clone());
    let global_bar = create_row(Some("global"), tx2.clone(), true);
    let app_box = Box::new(Orientation::Vertical, 10);

    // make the app box vertically scrollable
    let scrolled_box: ScrolledWindow = ScrolledWindow::new::<Adjustment, Adjustment>(None, None);
    scrolled_box.set_property_hscrollbar_policy(PolicyType::Never);
    scrolled_box.add(&app_box);

    main_box.add(&interface_row);
    main_box.add(&global_bar);
    main_box.pack_end(&scrolled_box, true, true, 10);
    window.add(&main_box);
    window.show_all();

    // spawn limiter thread
    thread::spawn(|| {
        if let Err(e) = limit::limit(Some(2), tx, rx2) {
            panic!("Something happened: {}", e);
        }
        // if limiter finishes
        // then its time to exit
        std::process::exit(0)
    });

    // If nethogs is installed on the system
    // spawn nethogs thread
    if check_for_dependencies(&["nethogs"]).is_ok() {
        thread::spawn(|| {
            if let Err(e) = nethogs(tx_c) {
                panic!("Nethogs error: {}", e);
            }
        });
    }

    // callback to add new programs to gui
    rx.attach(None, move |message| {
        match message {
            UpdateGuiMessage::ProgramEntry(program) => {
                let app_bar = create_row(Some(&program), tx2.clone(), false);
                app_box.add(&app_bar);
                app_box.show_all();
            }
            UpdateGuiMessage::CurrentProgramSpeed(prgoram_current_speed) => {
                update_gui_program_speed(app_box.clone(), prgoram_current_speed);
            }
            UpdateGuiMessage::CurrentGlobalSpeed(global_speed) => {
                update_gui_global_speed(global_bar.clone(), global_speed);
            }
        }

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

pub enum UpdateGuiMessage {
    ProgramEntry(String),
    CurrentProgramSpeed(HashMap<String, (f32, f32)>),
    CurrentGlobalSpeed((f32, f32)),
}
