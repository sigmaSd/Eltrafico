mod tc;
mod utils;
use crate::tc::*;
use crate::utils::ss;
use clap::Parser;
use serde::Deserialize;
// use serde_derive::Deserialize;

use simple_logger::SimpleLogger;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};

use ctrlc;
use notify::{watcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;

use std::time::Duration;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser, Debug)]
#[clap(
    author = "sigmaSd",
    version = "2.3.1",
    about = "Eltrafico",
    long_about = "NetLimiter-like traffic shaping for Linux"
)]
struct Args {
    /// Name of the person to greet
    #[clap(short, long, value_parser)]
    config: String,
    #[clap(short, long = "delay on shaping", value_parser)]
    delay: Option<usize>,
}

#[derive(Deserialize)]
struct LimitConf {
    download: Option<String>,
    upload: Option<String>,
    download_minimum: Option<String>,
    upload_minimum: Option<String>,
    download_priority: Option<usize>,
    upload_priority: Option<usize>,
    rule_name: Option<String>,
    match_exe: Option<String>,
    interface: Option<String>,
}
#[derive(Deserialize)]
struct Config {
    global: LimitConf,
    process: Vec<LimitConf>,
}
fn read_config(config_path: &str) -> std::io::Result<Config> {
    let content = std::fs::read_to_string(config_path)?;
    Ok(toml::from_str(&content)?)
}
fn main() {
    SimpleLogger::new().init().unwrap();

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"-c".to_string()) {
        let args = Args::parse();
        if Path::new(args.config.as_str()).exists() {
            reset_on_exit(args.config.clone()).unwrap();
            limit_conf(args.delay, args.config.as_str()).unwrap();
        } else {
            log::info!("Loading config from {} failed !", args.config);
        }
    } else {
        limit(Some(2), io::stdout(), io::stdin()).unwrap();
    }
}
fn load_conf(config_path: String) -> crate::Result<Config> {
    log::info!("Loading config from {}!", config_path);
    let config: Config = read_config(config_path.as_str()).unwrap();
    log::info!(
        "Global: {:?} {:?} {:?} {:?} {:?} {:?} {:?}  ",
        config.global.interface,
        config.global.download,
        config.global.upload,
        config.global.download_minimum,
        config.global.upload_minimum,
        config.global.download_priority,
        config.global.upload_priority
    );
    for proc in config.process.iter() {
        log::info!(
            "process: {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}  ",
            proc.rule_name,
            proc.match_exe,
            proc.download,
            proc.upload,
            proc.download_minimum,
            proc.upload_minimum,
            proc.download_priority,
            proc.upload_priority
        );
    }
    Ok(config)
}

fn reset_on_exit(config_path: String) -> crate::Result<()> {
    ctrlc::set_handler(move || {
        println!("received Ctrl+C! cleaning...");
        println!("Loading config from {}!", config_path);
        let config: Config = read_config(config_path.as_str()).unwrap();
        let current_interface = config.global.interface.unwrap();
        // let max_rate_o = Some(String::from("4294967295"));
        // let min_rate_o = Some(String::from("8"));
        let (mut root_ingress, mut root_egress) = tc_setup(
            current_interface.clone(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let mut filtered_ports: HashMap<(TrafficType, String), String> = HashMap::new();
        clean_up(&root_ingress.device, &current_interface).unwrap();
        reset_tc(
            &current_interface,
            &mut root_ingress,
            &mut root_egress,
            (None, None, None, None),
            &mut filtered_ports,
        )
        .unwrap();
        exit(0)
    })
    .expect("Error setting Ctrl-C handler");
    Ok(())
}

fn reconfigure(
    root_ingress: &mut QDisc,
    current_interface: String,
    root_egress: &mut QDisc,
    config: Config,
    filtered_ports: &mut HashMap<(TrafficType, String), String>,
    program_to_trafficid_map: &mut HashMap<String, (Option<usize>, Option<usize>)>,
) -> crate::Result<()> {
    log::info!("reconfiguring...");
    clean_up(&root_ingress.device, &current_interface)?;

    // save new values

    reset_tc(
        &current_interface,
        root_ingress,
        root_egress,
        (
            config.global.download.clone(),
            config.global.upload.clone(),
            config.global.download_minimum.clone(),
            config.global.upload_minimum.clone(),
        ),
        filtered_ports,
    )?;

    // process
    for proc in config.process {
        let ingress_class_id =
            if let (Some(down), Some(down_min)) = (proc.download, proc.download_minimum) {
                Some(tc::tc_add_htb_class(
                    &root_ingress,
                    Some(down),
                    Some(down_min),
                    proc.download_priority,
                )?)
            } else {
                None
            };

        let egress_class_id = if let (Some(up), Some(up_min)) = (proc.upload, proc.upload_minimum) {
            Some(tc::tc_add_htb_class(
                &root_egress,
                Some(up),
                Some(up_min),
                proc.upload_priority,
            )?)
        } else {
            None
        };
        program_to_trafficid_map.insert(
            proc.match_exe.clone().unwrap(),
            (ingress_class_id, egress_class_id),
        );
    }
    Ok(())
}
pub fn limit_conf(delay: Option<usize>, config_path: &str) -> crate::Result<()> {
    use TrafficType::*;
    let config_path_str = config_path.to_string();
    let config: Config = load_conf(config_path_str.clone())?;

    let (tx, rx) = channel();

    // Create a watcher object, delivering debounced events.
    // The notification back-end is selected based on the platform.
    let mut watcher = watcher(tx, Duration::from_secs(10)).unwrap();

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher
        .watch(config_path, RecursiveMode::NonRecursive)
        .unwrap();

    let current_interface = config.global.interface.clone().unwrap();
    let program_to_trafficid_map = HashMap::new();
    let (root_ingress, root_egress) = tc_setup(
        current_interface.clone(),
        config.global.download.clone(),
        config.global.download_minimum.clone(),
        config.global.upload.clone(),
        config.global.upload_minimum.clone(),
        None,
        None,
    )?;
    let root_ingress_arc = Arc::new(Mutex::new(root_ingress));
    let root_egress_arc = Arc::new(Mutex::new(root_egress));

    let filtered_ports: HashMap<(TrafficType, String), String> = HashMap::new();

    let filtered_ports_arc = Arc::new(Mutex::new(filtered_ports));
    let program_to_trafficid_map_arc = Arc::new(Mutex::new(program_to_trafficid_map));
    reconfigure(
        &mut root_ingress_arc.lock().unwrap(),
        current_interface.clone(),
        &mut root_egress_arc.lock().unwrap(),
        config,
        &mut filtered_ports_arc.lock().unwrap(),
        &mut program_to_trafficid_map_arc.lock().unwrap(),
    )?;
    let root_ingress_c = Arc::clone(&root_ingress_arc);
    let root_egress_c = Arc::clone(&root_egress_arc);
    let filtered_ports_c = Arc::clone(&filtered_ports_arc);
    let program_to_trafficid_map_c = Arc::clone(&program_to_trafficid_map_arc);
    let config_reloader = std::thread::spawn({
        move || loop {
            match rx.recv() {
                Ok(event) => {
                    log::info!("{:?}", event);
                    let config = load_conf(config_path_str.clone()).unwrap();
                    reconfigure(
                        &mut root_ingress_c.lock().unwrap(),
                        current_interface.clone(),
                        &mut root_egress_c.lock().unwrap(),
                        config,
                        &mut filtered_ports_c.lock().unwrap(),
                        &mut program_to_trafficid_map_c.lock().unwrap(),
                    )
                    .unwrap();
                }
                Err(e) => log::info!("watch error: {:?}", e),
            }
        }
    });
    let root_ingress_c = Arc::clone(&root_ingress_arc);
    let root_egress_c = Arc::clone(&root_egress_arc);
    let filtered_ports_c = Arc::clone(&filtered_ports_arc);
    let program_to_trafficid_map_c = Arc::clone(&program_to_trafficid_map_arc);
    let traffic_shaper = std::thread::spawn(move || loop {
        // writeln!(ioout, "here11111")?;
        let active_connections = ss().unwrap();
        let mut active_ports = HashMap::new();
        for (program, connections) in active_connections {
            let program_in_map = program_to_trafficid_map_c
                .lock()
                .unwrap()
                .get(&program)
                .map(ToOwned::to_owned);
            let (ingress_class_id, egress_class_id) = match program_in_map {
                Some(id) => id,
                None => {
                    // this is a new program
                    // add a placeholder for it in the program_to_trafficid_map
                    // and send it to the gui
                    program_to_trafficid_map_c
                        .lock()
                        .unwrap()
                        .insert(program.clone(), (None, None));

                    let msg = format!("ProgramEntry: {}", program);
                    // writeln!(ioout, "{}", msg)?;
                    log::info!("{}", msg);
                    continue;
                }
            };

            // filter the connection ports accoding the user specified limits
            for con in connections {
                if let Some(ingress_class_id) = ingress_class_id {
                    let ingress_port = (Ingress, con.lport.clone());

                    if filtered_ports_c.lock().unwrap().contains_key(&ingress_port) {
                        active_ports.insert(
                            ingress_port.clone(),
                            filtered_ports_c.lock().unwrap()[&ingress_port].clone(),
                        );
                        continue;
                    } else {
                        let ingress_filter_id = add_ingress_filter(
                            con.lport.parse().unwrap(),
                            &root_ingress_c.lock().unwrap(),
                            ingress_class_id,
                        )
                        .unwrap();
                        active_ports.insert(ingress_port, ingress_filter_id);
                    }
                }

                if let Some(egress_class_id) = egress_class_id {
                    let egress_port = (Egress, con.lport.clone());

                    if filtered_ports_c.lock().unwrap().contains_key(&egress_port) {
                        active_ports.insert(
                            egress_port.clone(),
                            filtered_ports_c.lock().unwrap()[&egress_port].clone(),
                        );
                        continue;
                    } else {
                        let egress_filter_id = add_egress_filter(
                            con.lport.parse().unwrap(),
                            &root_egress_c.lock().unwrap(),
                            egress_class_id,
                        )
                        .unwrap();
                        active_ports.insert(egress_port, egress_filter_id);
                    }
                }
            }
        }

        // remove filter for freed ports
        for (port, filter_id) in filtered_ports_c.lock().unwrap().clone() {
            if !active_ports.contains_key(&port) {
                match port.0 {
                    Ingress => {
                        tc::tc_remove_u32_filter(&root_ingress_c.lock().unwrap(), filter_id)
                            .unwrap();
                    }
                    Egress => {
                        tc::tc_remove_u32_filter(&root_egress_c.lock().unwrap(), filter_id)
                            .unwrap();
                    }
                }
            }
        }

        // update the currently filtered ports
        filtered_ports_c.lock().unwrap().clear();
        filtered_ports_c
            .lock()
            .unwrap()
            .extend(active_ports.into_iter());

        // delay scanning for active connections
        if let Some(delay) = delay {
            // log::info!("delay sleep: {}", delay);
            std::thread::sleep(std::time::Duration::from_secs(delay as u64));
        }
    });
    config_reloader.join().unwrap();
    traffic_shaper.join().unwrap();
    Ok(())
}
pub fn limit(delay: Option<usize>, mut tx: io::Stdout, rx: io::Stdin) -> crate::Result<()> {
    use TrafficType::*;

    let mut program_to_trafficid_map = HashMap::new();

    // block till we get an initial interface
    // and while we're at it if we get a global limit msg save the values
    // also if we get stop msg quit early
    let mut global_limit_record: (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = Default::default();
    let mut msg = String::new();

    let mut current_interface = loop {
        rx.read_line(&mut msg)?;
        if !msg.is_empty() {
            match msg.clone().into() {
                Message::Stop => {
                    writeln!(tx, "Stop")?;
                    return Ok(());
                }
                Message::Interface(name) => break name,
                Message::Global(limit) => {
                    global_limit_record = limit;
                }
                Message::Program(_) => (),
            }
            msg.clear();
        }
    };

    let (mut root_ingress, mut root_egress) = tc_setup(
        current_interface.clone(),
        global_limit_record.0.clone(),
        global_limit_record.2.clone(),
        global_limit_record.1.clone(),
        global_limit_record.3.clone(),
        None,
        None,
    )?;

    let mut filtered_ports: HashMap<(TrafficType, String), String> = HashMap::new();

    let msgs = Arc::new(Mutex::new(String::new()));

    // Read stdin msg in a new thread
    let msgs_c = msgs.clone();
    std::thread::spawn(move || {
        let mut tmp = String::new();
        loop {
            rx.read_line(&mut tmp)
                .expect("Error reading message from eltrfico");
            *msgs_c.lock().unwrap() = tmp.clone();
            tmp.clear();
        }
    });

    loop {
        // check for new user limits
        // and add htb class for them
        // TODO remove freed htb classes
        let mut active_ports = HashMap::new();

        // check if we recieved a new msg on stdin
        let msg = msgs.lock().unwrap().clone();
        if !msg.is_empty() {
            match Message::from(msg) {
                Message::Interface(name) => {
                    clean_up(&root_ingress.device, &current_interface)?;

                    current_interface = name;
                    reset_tc(
                        &current_interface,
                        &mut root_ingress,
                        &mut root_egress,
                        global_limit_record.clone(),
                        &mut filtered_ports,
                    )?;
                }
                Message::Global(limit) => {
                    clean_up(&root_ingress.device, &current_interface)?;

                    // save new values
                    global_limit_record = limit;

                    reset_tc(
                        &current_interface,
                        &mut root_ingress,
                        &mut root_egress,
                        global_limit_record.clone(),
                        &mut filtered_ports,
                    )?;
                }
                Message::Program((name, (down, up, down_min, up_min))) => {
                    let ingress_class_id = if let (Some(down), Some(down_min)) = (down, down_min) {
                        Some(tc::tc_add_htb_class(
                            &root_ingress,
                            Some(down),
                            Some(down_min),
                            None,
                        )?)
                    } else {
                        None
                    };

                    let egress_class_id = if let (Some(up), Some(up_min)) = (up, up_min) {
                        Some(tc::tc_add_htb_class(
                            &root_egress,
                            Some(up),
                            Some(up_min),
                            None,
                        )?)
                    } else {
                        None
                    };

                    program_to_trafficid_map
                        .insert(name.clone(), (ingress_class_id, egress_class_id));
                }
                Message::Stop => {
                    clean_up(&root_ingress.device, &current_interface)?;
                    writeln!(tx, "Stop")?;
                    break Ok(());
                }
            }
            // clear msg
            msgs.lock().unwrap().clear();
        }

        // look for new ports to filter
        let active_connections = ss()?;

        for (program, connections) in active_connections {
            let program_in_map = program_to_trafficid_map
                .get(&program)
                .map(ToOwned::to_owned);
            let (ingress_class_id, egress_class_id) = match program_in_map {
                Some(id) => id,
                None => {
                    // this is a new program
                    // add a placeholder for it in the program_to_trafficid_map
                    // and send it to the gui
                    program_to_trafficid_map.insert(program.clone(), (None, None));

                    let msg = format!("ProgramEntry: {}", program);
                    writeln!(tx, "{}", msg)?;
                    continue;
                }
            };

            // filter the connection ports accoding the user specified limits
            for con in connections {
                if let Some(ingress_class_id) = ingress_class_id {
                    let ingress_port = (Ingress, con.lport.clone());

                    if filtered_ports.contains_key(&ingress_port) {
                        active_ports
                            .insert(ingress_port.clone(), filtered_ports[&ingress_port].clone());
                        continue;
                    } else {
                        let ingress_filter_id = add_ingress_filter(
                            con.lport.parse().unwrap(),
                            &root_ingress,
                            ingress_class_id,
                        )?;
                        active_ports.insert(ingress_port, ingress_filter_id);
                    }
                }

                if let Some(egress_class_id) = egress_class_id {
                    let egress_port = (Egress, con.lport.clone());

                    if filtered_ports.contains_key(&egress_port) {
                        active_ports
                            .insert(egress_port.clone(), filtered_ports[&egress_port].clone());
                        continue;
                    } else {
                        let egress_filter_id = add_egress_filter(
                            con.lport.parse().unwrap(),
                            &root_egress,
                            egress_class_id,
                        )?;
                        active_ports.insert(egress_port, egress_filter_id);
                    }
                }
            }
        }

        // remove filter for freed ports
        for (port, filter_id) in filtered_ports {
            if !active_ports.contains_key(&port) {
                match port.0 {
                    Ingress => {
                        tc::tc_remove_u32_filter(&root_ingress, filter_id)?;
                    }
                    Egress => {
                        tc::tc_remove_u32_filter(&root_egress, filter_id)?;
                    }
                }
            }
        }

        // update the currently filtered ports
        filtered_ports = active_ports;

        // delay scanning for active connections
        if let Some(delay) = delay {
            std::thread::sleep(std::time::Duration::from_secs(delay as u64));
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
enum TrafficType {
    /// Incomming traffic
    Ingress,
    /// Outgoing traffic
    Egress,
}

fn reset_tc(
    current_interface: &str,
    ingress: &mut QDisc,
    egress: &mut QDisc,
    global_limit: (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ),
    filtered_ports: &mut HashMap<(TrafficType, String), String>,
) -> Result<()> {
    filtered_ports.clear();

    let (new_ingress, new_egress) = tc::tc_setup(
        current_interface.into(),
        global_limit.0,
        global_limit.2,
        global_limit.1,
        global_limit.3,
        None,
        None,
    )?;

    *ingress = new_ingress;
    *egress = new_egress;

    Ok(())
}

#[derive(Eq, PartialEq, Debug)]
pub enum Message {
    Stop,
    Interface(String),
    Global(
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    ),
    Program(
        (
            String,
            (
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        ),
    ),
}

impl From<String> for Message {
    fn from(msg: String) -> Message {
        let parse = || -> Option<Message> {
            use Message::*;
            match msg.trim() {
                "Stop" => Some(Stop),
                msg if msg.starts_with("Interface: ") => {
                    Some(Interface(msg.split("Interface: ").nth(1)?.to_string()))
                }
                msg if msg.starts_with("Global: ") => {
                    let msg = msg.split("Global: ").nth(1)?;
                    let mut msg = msg.split_whitespace();
                    let mut down = msg.next().map(ToString::to_string);
                    let mut up = msg.next().map(ToString::to_string);
                    let mut down_min = msg.next().map(ToString::to_string);
                    let mut up_min = msg.next().map(ToString::to_string);
                    if down == Some("None".into()) {
                        down = None;
                    }
                    if up == Some("None".into()) {
                        up = None;
                    }

                    if down_min == Some("None".into()) {
                        down_min = None;
                    }

                    if up_min == Some("None".into()) {
                        up_min = None;
                    }

                    Some(Global((down, up, down_min, up_min)))
                }
                msg if msg.starts_with("Program: ") => {
                    let msg = msg.split("Program: ").nth(1)?;
                    let mut msg = msg.split_whitespace();
                    let program_name = msg.next()?.to_string();
                    let mut down = msg.next().map(ToString::to_string);
                    let mut up = msg.next().map(ToString::to_string);
                    let mut down_min = msg.next().map(ToString::to_string);
                    let mut up_min = msg.next().map(ToString::to_string);
                    if down == Some("None".into()) {
                        down = None;
                    }
                    if up == Some("None".into()) {
                        up = None;
                    }
                    if down_min == Some("None".into()) {
                        down_min = None;
                    }
                    if up_min == Some("None".into()) {
                        up_min = None;
                    }

                    Some(Program((program_name, (down, up, down_min, up_min))))
                }
                msg => panic!("Uknown msg recieved: {}", msg),
            }
        };
        parse().unwrap_or_else(|| panic!("Malformated message: {}", msg))
    }
}

fn clean_up(ingress_device: &str, egress_device: &str) -> Result<()> {
    log::info!("Cleaning up QDiscs");
    tc_remove_qdisc(ingress_device.into(), None)?;
    tc_remove_qdisc(egress_device.into(), None)?;
    tc_remove_qdisc(egress_device.into(), Some(INGRESS_QDISC_PARENT_ID.into()))?;
    Ok(())
}

fn add_ingress_filter(
    port: usize,
    ingress_qdisc: &QDisc,
    class_id: usize,
) -> crate::Result<String> {
    let filter_id = tc_add_u32_filter(
        ingress_qdisc,
        format!("match ip dport {port} 0xffff"),
        class_id,
    )?;
    Ok(filter_id)
}

fn add_egress_filter(port: usize, egress_qdisc: &QDisc, class_id: usize) -> Result<String> {
    let filter_id = tc_add_u32_filter(
        egress_qdisc,
        format!("match ip sport {port} 0xffff"),
        class_id,
    )?;
    Ok(filter_id)
}
