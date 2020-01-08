use crate::gui::Message;
use crate::tc::{self, *};
use crate::utils::ss;
use crate::CatchAll;
use glib::Sender;
use std::collections::HashMap;
use std::sync::mpsc;

pub fn limit(
    delay: Option<usize>,
    tx: Sender<String>,
    rx: mpsc::Receiver<Message>,
) -> crate::CatchAll<()> {
    use TrafficType::*;

    let mut program_to_trafficid_map = HashMap::new();

    // block till we get an initial interface
    // and while we're at it if we get a global limit msg save the values
    // also if we get stop msg quit early
    let mut global_limit_record: (Option<String>, Option<String>) = Default::default();
    let mut current_interface = loop {
        if let Ok(msg) = rx.recv() {
            match msg {
                Message::Stop => return Ok(()),
                Message::Interface(name) => break name,
                Message::Global(limit) => {
                    global_limit_record = limit;
                }
                Message::Program(_) => (),
            }
        }
    };

    let (mut root_ingress, mut root_egress) = tc_setup(
        &current_interface,
        global_limit_record.0.clone(),
        global_limit_record.1.clone(),
    )?;

    let mut filtered_ports: HashMap<(TrafficType, String), String> = HashMap::new();

    loop {
        let mut active_ports = HashMap::new();

        let msgs: Vec<Message> = rx.try_iter().collect();
        // if a stop msg is in the channel queue
        // dont go through the whole queue
        // clean it up then quit
        if msgs.iter().any(|msg| msg == &Message::Stop) {
            tc::clean_up(&root_ingress.interface, &current_interface)?;
            break Ok(());
        }
        // check for new user limits
        // and add htb class for them
        // TODO remove freed htb classes
        for msg in msgs {
            match msg {
                Message::Interface(name) => {
                    tc::clean_up(&root_ingress.interface, &current_interface)?;

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
                    tc::clean_up(&root_ingress.interface, &current_interface)?;

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
                Message::Program((name, (down, up))) => {
                    let ingress_class_id = if let Some(down) = down {
                        Some(tc::tc_add_htb_class(&root_ingress, &down)?)
                    } else {
                        None
                    };

                    let egress_class_id = if let Some(up) = up {
                        Some(tc::tc_add_htb_class(&root_egress, &up)?)
                    } else {
                        None
                    };

                    program_to_trafficid_map
                        .insert(name.clone(), (ingress_class_id, egress_class_id));
                }
                // Already handled
                Message::Stop => unreachable!(),
            }
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
                    // add a placeholder for in the program_to_trafficid_map
                    // and send it to the gui
                    program_to_trafficid_map.insert(program.clone(), (None, None));
                    tx.send(program.clone())
                        .expect("failed to send data to the main thread");
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
                        let ingress_filter_id = tc::add_ingress_filter(
                            &con.lport,
                            &root_ingress.interface,
                            root_ingress.qdisc_id,
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
                        let egress_filter_id = tc::add_egress_filter(
                            &con.lport,
                            &root_egress.interface,
                            root_egress.qdisc_id,
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
                        tc::tc_remove_u32_filter(
                            &root_ingress.interface,
                            &filter_id,
                            root_ingress.qdisc_id,
                        )?;
                    }
                    Egress => {
                        tc::tc_remove_u32_filter(
                            &root_egress.interface,
                            &filter_id,
                            root_egress.qdisc_id,
                        )?;
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
    ingress: &mut Traffic,
    egress: &mut Traffic,
    global_limit: (Option<String>, Option<String>),
    filtered_ports: &mut HashMap<(TrafficType, String), String>,
) -> CatchAll<()> {
    filtered_ports.clear();

    let (new_ingress, new_egress) =
        tc::tc_setup(current_interface, global_limit.0, global_limit.1)?;

    *ingress = new_ingress;
    *egress = new_egress;

    Ok(())
}
