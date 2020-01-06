use crate::limit;
use crate::tc;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::*;
use std::collections::HashMap;
use std::env::args;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

fn build_ui(application: &gtk::Application) {
    let programs_to_limit = Arc::new(Mutex::new(HashMap::new()));
    let programs_to_limit_c = programs_to_limit.clone();
    // ui build
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("ElTrafico");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(300, 500);

    let main_box = Box::new(Orientation::Vertical, 10);
    let global_bar = create_global_row(programs_to_limit.clone());
    let app_box = Box::new(Orientation::Vertical, 10);
    let app_box_c = app_box.clone();
    //let currently_displayed_programs_to_limit = Arc::new(Mutex::new(vec![]));

    main_box.add(&global_bar);
    main_box.add(&app_box_c);
    window.add(&main_box);
    window.show_all();
    window.connect_delete_event(|_, _| {
        tc::clean_up("ifb0", "wlp3s0").unwrap();
        Inhibit(false)
    });

    // limit
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    use std::sync::mpsc;
    let (tx2, rx2) = mpsc::channel();

    thread::spawn(|| {
        limit::limit(
            programs_to_limit_c,
            "wlp3s0",
            Some(2),
            Arc::new(AtomicBool::new(true)),
            tx,
            rx2,
        )
        .unwrap();
    });

    rx.attach(None, move |program| {
        let app_bar = create_row(Some(&program), programs_to_limit.clone(), tx2.clone());
        app_box.add(&app_bar);
        app_box.show_all();
        // for i in app_box.get_children() {
        //     let name = i
        //         .clone()
        //         .downcast::<Box>()
        //         .unwrap()
        //         .get_children()
        //         .into_iter()
        //         .next()
        //         .unwrap()
        //         .downcast::<Label>()
        //         .unwrap()
        //         .get_text()
        //         .unwrap()
        //         .to_string();
        //     if !programs_to_limit.contains_key(&name) {
        //         app_box.remove(&i);
        //     }
        // }
        // for p in programs_to_limit.keys() {
        //     if currently_displayed_programs_to_limit.clone().borrow().contains(p) {
        //         continue;
        //     }
        //     let hbox = create_row(Some(&p), programs_to_limit.clone());
        //     app_box.add(&hbox);
        // }
        // app_box.show_all();
        // *currently_displayed_programs_to_limitlock().unwrap() = programs_to_limit.keys().programs_to_limit(ToOwned::to_owned).collect();

        //limit::limit(programs_to_limit.clone(), programs_to_limit, ingress, egress);

        glib::Continue(true)
    });
}

fn create_global_row(_programs_to_limit: FilterMap) -> Box {
    let title = Label::new(Some("Global limit"));
    let down = Label::new(Some("Down: "));
    let down_value = Entry::new();
    down_value.set_placeholder_text(Some("None"));
    let up = Label::new(Some("Up: "));
    let up_value = Entry::new();
    up_value.set_placeholder_text(Some("None"));

    let set_btn = Button::new_with_label("set limit");

    let dv_c = down_value.clone();
    let uv_c = up_value.clone();

    set_btn.connect_clicked(move |_btn| {
        let _down = dv_c.get_text().map(|s| s.to_string());
        let _up = uv_c.get_text().map(|s| s.to_string());

        // Todo
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
type Rate = Option<String>;
pub type FilterMap = Arc<Mutex<HashMap<String, (Rate, Rate)>>>;
fn create_row(
    name: Option<&str>,
    _programs_to_limit: FilterMap,
    tx2: std::sync::mpsc::Sender<(String, (Rate, Rate))>,
) -> Box {
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
    let name = name.unwrap().to_string();
    set_btn.connect_clicked(move |_btn| {
        let down = d_c.get_text().unwrap().to_string();
        let down = if down.is_empty() { None } else { Some(down) };
        let up = u_c.get_text().unwrap().to_string();
        let up = if up.is_empty() { None } else { Some(up) };
        tx2.send((name.clone(), (down, up))).unwrap();
        // programs_to_limit
        //     .lock()
        //     .unwrap()
        //     .entry(name.clone())
        //     .or_insert((down, up));
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

pub fn run() {
    let application = gtk::Application::new(Some("com.github.eltrfico"), Default::default())
        .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&args().collect::<Vec<_>>());
}
