use crate::tc::{self, *};
use crate::{CatchAll, Rate};
use glib::Sender;
use std::collections::HashMap;
use std::sync::mpsc;

pub fn limit(
    delay: Option<usize>,
    tx: Sender<String>,
    rx: mpsc::Receiver<(String, (Rate, Rate))>,
) -> crate::CatchAll<()> {
    use TrafficType::*;

    let mut program_to_trafficid_map = HashMap::new();

    // block till we get an initial interface
    // and while we're at it if we get a global limit msg save the values
    let mut global_limit_record: (Rate, Rate) = Default::default();
    let mut current_interface = loop {
        if let Ok((msg_key, (msg_value1, msg_value2))) = rx.recv() {
            if msg_key == "stop" {
                return Ok(());
            }
            if msg_key == "interface" {
                break msg_value1;
            }
            if msg_key == "global" {
                global_limit_record = (msg_value1, msg_value2);
            }
        }
    };

    let (mut root_ingress, mut root_egress) = tc_setup(
        &current_interface
            .clone()
            .expect("Error reading interface name"),
        global_limit_record.0.clone(),
        global_limit_record.1.clone(),
    )?;

    let mut filtered_ports: HashMap<(TrafficType, String), String> = HashMap::new();

    loop {
        let active_connections = crate::lsof::lsof()?;
        let mut active_ports = HashMap::new();

        // check for new user limits
        // and add htb class for them
        // TODO remove freed htb classes
        let msgs: Vec<(String, (Rate, Rate))> = rx.try_iter().collect();
        // if a stop msg is in the channel queue
        // dont go through the whole queue
        // find last selected interface msg
        // clean it up then quit
        if msgs.iter().any(|(s, _)| s == "stop") {
            let last_selected_interface = msgs.iter().rfind(|(i, _)| i == "interface");
            if let Some(last_selected_interface) = last_selected_interface {
                let (_, (_, last_selected_interface_name)) = last_selected_interface;
                current_interface = last_selected_interface_name.clone();
            }
            tc::clean_up(
                &root_ingress.interface,
                &current_interface.expect("Error no interface is selected"),
            )?;
            break Ok(());
        }
        for msg in msgs {
            let (program, (down, up)) = msg;
            // look for a new selcted interface
            if program == "interface" {
                // down == up == interface_name
                current_interface = down;
                let interface_name = up.expect("Error reading interface name");
                reset_tc(
                    &interface_name,
                    &mut root_ingress,
                    &mut root_egress,
                    global_limit_record.clone(),
                    &mut filtered_ports,
                )?;
            }
            // look for a global limit
            else if program == "global" {
                // save new values
                global_limit_record = (down, up);

                reset_tc(
                    &current_interface
                        .clone()
                        .expect("Error no interface is selected"),
                    &mut root_ingress,
                    &mut root_egress,
                    global_limit_record.clone(),
                    &mut filtered_ports,
                )?;
            } else {
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
                    .insert(program.clone(), (ingress_class_id, egress_class_id));
            }
        }

        // look for new ports to filter
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
    global_limit: (Rate, Rate),
    filtered_ports: &mut HashMap<(TrafficType, String), String>,
) -> CatchAll<()> {
    tc::clean_up(&ingress.interface, current_interface)?;
    filtered_ports.clear();

    let (new_ingress, new_egress) =
        tc::tc_setup(current_interface, global_limit.0, global_limit.1)?;

    *ingress = new_ingress;
    *egress = new_egress;

    Ok(())
}
