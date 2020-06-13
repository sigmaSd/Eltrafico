mod widget_builder;
use crate::nethogs::nethogs;
use crate::run;
use crate::utils::check_for_dependencies;
use crate::utils::find_eltrafico_tc;
use gio::prelude::*;
use gtk::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::thread;
use widget_builder::*;

fn build_ui(application: &gtk::Application) {
    // channels
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let tx_c = tx.clone();

    // spawn tc thread
    let eltrafico_tc = find_eltrafico_tc().expect("Cannot find eltrafico_tc binary");
    let cmd = Command::new("pkexec")
        .arg(eltrafico_tc)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        // Debug
        //.stderr(log)
        .spawn()
        .expect("Error spawning eltrafico_tc");

    let stdin = Rc::new(RefCell::new(cmd.stdin));
    let mut stdout = cmd.stdout;

    // listen to tc thread stdout and send output to gui
    std::thread::spawn(move || {
        let mut tmp = String::new();
        let mut stdout = BufReader::new(stdout.as_mut().unwrap());

        loop {
            stdout
                .read_line(&mut tmp)
                .expect("Error reading message from eltrafico_tc");
            if tmp.is_empty() || tmp.trim() == "Stop" {
                tx_c.send(UpdateGuiMessage::Stop)
                    .expect("Error sending msg to gui thread");
            } else {
                tx_c.send(UpdateGuiMessage::ProgramEntry(tmp.trim().to_string()))
                    .expect("Error sending msg to gui thread");
            }
            tmp.clear();
        }
    });

    // If nethogs is installed on the system
    // spawn nethogs thread
    if check_for_dependencies(&["nethogs"]).is_ok() {
        thread::spawn(|| {
            if let Err(e) = nethogs(tx) {
                panic!("Nethogs error: {}", e);
            }
        });
    }

    // ui build
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("ElTrafico");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(300, 500);

    let main_box = Box::new(Orientation::Vertical, 10);
    let interface_row = create_interface_row(stdin.clone());
    let global_bar = create_row(Some("global"), stdin.clone(), true);
    let app_box = Box::new(Orientation::Vertical, 10);

    // make the app box vertically scrollable
    let scrolled_box: ScrolledWindow = ScrolledWindow::new::<Adjustment, Adjustment>(None, None);
    scrolled_box.set_property_hscrollbar_policy(PolicyType::Never);
    scrolled_box.add(&app_box);

    main_box.add(&interface_row);
    main_box.add(&global_bar);
    main_box.pack_end(&scrolled_box, true, true, 10);
    window.add(&main_box);

    // Cleanup at exit
    let stdin_c = stdin.clone();
    window.connect_delete_event(move |_, _| {
        // stop nethogs
        let pid = String::from_utf8(run!("pidof nethogs").unwrap().stdout).unwrap();
        if !pid.is_empty() {
            run!("pkexec pkill nethogs").expect("Error stopping nethogs");
        }
        // stop tc thread
        // tc will send a STOP msg back to the main thread so it can exit
        writeln!(stdin_c.borrow_mut().as_mut().unwrap(), "{}", Message::Stop)
            .expect("Error sending Stop message to eltrafico_tc");
        Inhibit(true)
    });

    // ctrlc not handled
    glib::source::unix_signal_add(2, move || panic!("Unclean exit!, ctrlc not handled"));

    // render gui
    window.show_all();

    // gui callbacks
    rx.attach(None, move |message| {
        match message {
            UpdateGuiMessage::CurrentProgramSpeed(prgoram_current_speed) => {
                update_gui_program_speed(app_box.clone(), prgoram_current_speed);
            }
            UpdateGuiMessage::CurrentGlobalSpeed(global_speed) => {
                update_gui_global_speed(global_bar.clone(), global_speed);
            }
            UpdateGuiMessage::ProgramEntry(program) => {
                if !program.is_empty() {
                    let stdin = stdin.clone();
                    let program = program
                        .split("ProgramEntry: ")
                        .nth(1)
                        .unwrap_or_else(|| panic!("Malformated message: {}", program));
                    let app_bar = create_row(Some(&program), stdin, false);
                    app_box.add(&app_bar);
                    app_box.show_all();
                }
            }
            UpdateGuiMessage::Stop => std::process::exit(0),
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

    application.run(&[]);
}

#[derive(PartialEq)]
pub enum Message {
    Stop,
    Interface(String),
    Global((Option<String>, Option<String>)),
    Program((String, (Option<String>, Option<String>))),
}

use std::fmt;
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Message::*;
        match self {
            Stop => write!(f, "Stop"),
            Interface(interface) => write!(f, "Interface: {}", interface),
            Global((up, down)) => {
                let mut msg = "Global: ".to_string();
                if let Some(up) = up {
                    msg.push_str(up);
                    msg.push(' ');
                } else {
                    msg.push_str("None ");
                }
                if let Some(down) = down {
                    msg.push_str(down);
                    msg.push(' ');
                } else {
                    msg.push_str("None ");
                }
                write!(f, "{}", msg)
            }
            Program((program, (up, down))) => {
                let mut msg = "Program: ".to_string();

                msg.push_str(program);
                msg.push(' ');
                if let Some(up) = up {
                    msg.push_str(up);
                    msg.push(' ');
                } else {
                    msg.push_str("None ");
                }
                if let Some(down) = down {
                    msg.push_str(down);
                    msg.push(' ');
                } else {
                    msg.push_str("None ");
                }
                write!(f, "{}", msg)
            }
        }
    }
}

#[derive(Debug)]
pub enum UpdateGuiMessage {
    Stop,
    ProgramEntry(String),
    CurrentProgramSpeed(HashMap<String, (f32, f32)>),
    CurrentGlobalSpeed((f32, f32)),
}
