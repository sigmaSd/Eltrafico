mod tc;
mod utils;
use crate::tc::{
    tc_add_htb_class, tc_add_u32_filter, tc_remove_qdisc, tc_remove_u32_filter, tc_setup, QDisc,
    INGRESS_QDISC_PARENT_ID,
};
use crate::utils::ss;
use log::{info, trace, warn};
use simple_logger::SimpleLogger;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::mpsc;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    SimpleLogger::new().init().unwrap();

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"-h".to_string()) || args.contains(&"--help".to_string()) {
        //TODO helpful message
        std::process::exit(0);
    }
    limit(Some(Duration::from_secs(2)), io::stdout(), io::stdin())
}

pub fn limit(delay: Option<Duration>, mut stdout: io::Stdout, stdin: io::Stdin) -> Result<()> {
    // block till we get an initial interface
    // and while we're at it if we get a global limit msg save the values
    // also if we get stop msg quit early
    let mut global_limit_record: (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = Default::default();

    trace!("waiting for interface");
    let mut current_interface = {
        let mut msg = String::new();
        loop {
            if stdin.read_line(&mut msg).is_err() {
                return Ok(());
            }
            trace!("recieved message: {}", msg.trim());
            match msg.clone().try_into() {
                Ok(msg) => match msg {
                    Message::Stop => {
                        writeln!(stdout, "Stop")?;
                        return Ok(());
                    }
                    Message::Interface(name) => break name,
                    Message::Global(limit) => {
                        global_limit_record = limit;
                    }
                    Message::Program(_) => (),
                },
                Err(e) => warn!("{e}"),
            }
            msg.clear();
        }
    };
    trace!("selected interface is {current_interface}");

    trace!("running tc_setup");
    let (mut root_ingress, mut root_egress) = tc_setup(
        current_interface.clone(),
        global_limit_record.0.clone(),
        global_limit_record.2.clone(),
        global_limit_record.1.clone(),
        global_limit_record.3.clone(),
        None,
        None,
    )?;

    handle_ctrlc(root_ingress.clone(), current_interface.clone());

    let (tx_stdin, rx_stdin) = mpsc::channel();

    std::thread::spawn(move || {
        let mut input = String::new();
        loop {
            stdin
                .read_line(&mut input)
                .expect("Error reading message from eltrfico");
            tx_stdin.send(input.clone()).unwrap();
            input.clear();
        }
    });

    let mut program_to_trafficid_map = HashMap::new();
    let mut filtered_ports: HashMap<DirPort, String> = HashMap::new();

    loop {
        // check for new user limits
        // and add htb class for them

        // check if we received a new msg on stdin
        if let Ok(msg) = rx_stdin.try_recv() {
            match Message::try_from(msg) {
                Ok(msg) => match msg {
                    Message::Interface(name) => {
                        info!("recieved interface: {name}");
                        clean_up(&root_ingress.device, &current_interface)?;

                        current_interface = name;
                        resetup_tc_and_filtered_ports(
                            &current_interface,
                            &mut root_ingress,
                            &mut root_egress,
                            global_limit_record.clone(),
                            &mut filtered_ports,
                        )?;
                    }
                    Message::Global(limit) => {
                        info!("recieved global limit: {limit:?}");
                        clean_up(&root_ingress.device, &current_interface)?;

                        // save new values
                        global_limit_record = limit;

                        resetup_tc_and_filtered_ports(
                            &current_interface,
                            &mut root_ingress,
                            &mut root_egress,
                            global_limit_record.clone(),
                            &mut filtered_ports,
                        )?;
                    }
                    Message::Program((name, (down, up, down_min, up_min))) => {
                        info!("recieved program: {name:?} {down:?} {up:?} {down_min:?} {up_min:?}");
                        let ingress_class_id = if let Some(down) = down {
                            Some(tc_add_htb_class(&root_ingress, Some(down), down_min, None)?)
                        } else {
                            None
                        };

                        let egress_class_id = if let Some(up) = up {
                            Some(tc_add_htb_class(&root_egress, Some(up), up_min, None)?)
                        } else {
                            None
                        };

                        program_to_trafficid_map
                            .insert(name.clone(), (ingress_class_id, egress_class_id));
                    }
                    Message::Stop => {
                        info!("recieved Stop");
                        clean_up(&root_ingress.device, &current_interface)?;
                        writeln!(stdout, "Stop")?;
                        break Ok(());
                    }
                },
                Err(e) => log::warn!("{e}"),
            }
        }

        // look for new ports to filter
        let mut active_ports = HashMap::new();
        for (program, connections) in ss()? {
            let program_in_map = program_to_trafficid_map
                .get(&program)
                .map(ToOwned::to_owned);
            let (ingress_class_id, egress_class_id) = match program_in_map {
                Some(id) => id,
                None => {
                    trace!("detected a new program {program}");
                    // this is a new program
                    // add a placeholder for it in the program_to_trafficid_map
                    // and send it to the gui
                    program_to_trafficid_map.insert(program.clone(), (None, None));
                    writeln!(stdout, "ProgramEntry: {program}")?;
                    continue;
                }
            };

            // filter the connection ports according the user specified limits
            for connection in connections {
                if let Some(ingress_class_id) = ingress_class_id {
                    let ingress_port = DirPort::Ingress(connection.lport);

                    if filtered_ports.contains_key(&ingress_port) {
                        active_ports.insert(ingress_port, filtered_ports[&ingress_port].clone());
                    } else {
                        trace!(
                            "adding a new ingress filter for port {} of connection {connection:?}",
                            connection.lport
                        );
                        let ingress_filter_id =
                            add_ingress_filter(connection.lport, &root_ingress, ingress_class_id)?;
                        active_ports.insert(ingress_port, ingress_filter_id);
                    }
                }

                if let Some(egress_class_id) = egress_class_id {
                    let egress_port = DirPort::Egress(connection.lport);

                    if filtered_ports.contains_key(&egress_port) {
                        active_ports.insert(egress_port, filtered_ports[&egress_port].clone());
                    } else {
                        trace!(
                            "adding a new egress filter for port {} of connection {connection:?}",
                            connection.lport
                        );
                        let egress_filter_id =
                            add_egress_filter(connection.lport, &root_egress, egress_class_id)?;
                        active_ports.insert(egress_port, egress_filter_id);
                    }
                }
            }
        }

        // remove filter for freed ports
        for (port, filter_id) in filtered_ports {
            if !active_ports.contains_key(&port) {
                match port {
                    DirPort::Ingress(_) => {
                        trace!("removing freed ingress port {port:?}");
                        tc_remove_u32_filter(&root_ingress, filter_id)?;
                    }
                    DirPort::Egress(_) => {
                        trace!("removing freed egress port {port:?}");
                        tc_remove_u32_filter(&root_egress, filter_id)?;
                    }
                }
            }
        }

        // update the currently filtered ports
        filtered_ports = active_ports;

        // delay scanning for active connections
        if let Some(delay) = delay {
            std::thread::sleep(delay);
        }
    }
}

/// Port with direction
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
enum DirPort {
    /// Incomming traffic
    Ingress(usize),
    /// Outgoing traffic
    Egress(usize),
}

fn resetup_tc_and_filtered_ports(
    current_interface: &str,
    ingress: &mut QDisc,
    egress: &mut QDisc,
    global_limit: (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ),
    filtered_ports: &mut HashMap<DirPort, String>,
) -> Result<()> {
    filtered_ports.clear();

    let (new_ingress, new_egress) = tc_setup(
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

impl TryFrom<String> for Message {
    type Error = String;
    fn try_from(msg: String) -> std::result::Result<Self, Self::Error> {
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
                _ => None,
            }
        };
        parse().ok_or(format!("failed to parse message: {msg}"))
    }
}

fn clean_up(ingress_device: &str, egress_device: &str) -> Result<()> {
    log::info!("Cleaning up QDiscs");
    tc_remove_qdisc(ingress_device.into(), None)?;
    tc_remove_qdisc(egress_device.into(), None)?;
    tc_remove_qdisc(egress_device.into(), Some(INGRESS_QDISC_PARENT_ID.into()))?;
    Ok(())
}

fn add_ingress_filter(port: usize, ingress_qdisc: &QDisc, class_id: usize) -> Result<String> {
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

fn handle_ctrlc(root_ingress: QDisc, current_interface: String) {
    ctrlc::set_handler(move || {
        log::warn!("Caught SIGINT signal");
        let _ = clean_up(&root_ingress.device, &current_interface);
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
}
