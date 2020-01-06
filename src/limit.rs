use crate::tc;
use crate::Rate;
use glib::Sender;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;

pub fn limit(
    interface: &str,
    delay: Option<usize>,
    tx: Sender<String>,
    rx: mpsc::Receiver<(String, (Rate, Rate))>,
) -> crate::CatchAll<()> {
    use TrafficType::*;

    let program_to_trafficid_map = Rc::new(RefCell::new(HashMap::new()));
    let program_to_trafficid_map_c = program_to_trafficid_map.clone();

    let (ingress, egress) = tc::tc_setup(interface, None, None)?;

    let (ingress_interface, ingress_qdisc_id, ingress_root_class_id) = ingress;
    let (egress_interface, egress_qdisc_id, egress_root_class_id) = egress;

    let ingress_interface = Rc::new(RefCell::new(ingress_interface));
    let ingress_qdisc_id = Rc::new(RefCell::new(ingress_qdisc_id));
    let ingress_root_class_id = Rc::new(RefCell::new(ingress_root_class_id));

    let egress_interface = Rc::new(RefCell::new(egress_interface));
    let egress_qdisc_id = Rc::new(RefCell::new(egress_qdisc_id));
    let egress_root_class_id = Rc::new(RefCell::new(egress_root_class_id));

    let egress_interface_c = egress_interface.clone();
    let ingress_interface_c = ingress_interface.clone();

    let mut filtered_ports: HashMap<(TrafficType, String), String> = HashMap::new();

    loop {
        let active_connections = crate::lsof::lsof()?;
        let mut active_ports = HashMap::new();

        // check for new user limits
        // and add htb class for them
        // TODO remove freed htb classes
        if let Ok(programs_to_limit) = rx.try_recv() {
            let (program, (down, up)) = programs_to_limit;
            if program == "global" {
                tc::clean_up("ifb0", "wlp3s0").unwrap();
                let (new_ingress, new_egress) = tc::tc_setup(interface, down, up)?;
                let (new_ingress_interface, new_ingress_qdisc_id, new_ingress_root_class_id) =
                    new_ingress;
                let (new_egress_interface, new_egress_qdisc_id, new_egress_root_class_id) =
                    new_egress;

                *ingress_interface.borrow_mut() = new_ingress_interface;
                *ingress_qdisc_id.borrow_mut() = new_ingress_qdisc_id;
                *ingress_root_class_id.borrow_mut() = new_ingress_root_class_id;
                *egress_interface.borrow_mut() = new_egress_interface;
                *egress_qdisc_id.borrow_mut() = new_egress_qdisc_id;
                *egress_root_class_id.borrow_mut() = new_egress_root_class_id;
            } else {
                let ingress_class_id = if let Some(down) = down {
                    Some(tc::tc_add_htb_class(
                        &ingress_interface_c.borrow(),
                        *ingress_qdisc_id.borrow(),
                        *ingress_root_class_id.borrow(),
                        &down,
                    )?)
                } else {
                    None
                };

                let egress_class_id = if let Some(up) = up {
                    Some(tc::tc_add_htb_class(
                        &egress_interface_c.borrow(),
                        *egress_qdisc_id.borrow(),
                        *egress_root_class_id.borrow(),
                        &up,
                    )?)
                } else {
                    None
                };

                program_to_trafficid_map_c
                    .borrow_mut()
                    .insert(program.clone(), (ingress_class_id, egress_class_id));
            }
        };

        let ingress_interface = ingress_interface.borrow().clone();
        let ingress_qdisc_id = *ingress_qdisc_id.borrow();
        let egress_interface = egress_interface.borrow().clone();
        let egress_qdisc_id = *egress_qdisc_id.borrow();

        // look for new ports to filter
        for (program, connections) in active_connections {
            let program_in_map = program_to_trafficid_map
                .borrow()
                .get(&program)
                .map(ToOwned::to_owned);
            let (ingress_class_id, egress_class_id) = match program_in_map {
                Some(id) => id,
                None => {
                    // this is a new program
                    // add a placeholder for in the program_to_trafficid_map
                    // and send it to the gui
                    program_to_trafficid_map
                        .borrow_mut()
                        .insert(program.clone(), (None, None));
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
                            &ingress_interface,
                            ingress_qdisc_id,
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
                            &egress_interface,
                            egress_qdisc_id,
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
                        tc::tc_remove_u32_filter(&ingress_interface, &filter_id, ingress_qdisc_id)?;
                    }
                    Egress => {
                        tc::tc_remove_u32_filter(&egress_interface, &filter_id, egress_qdisc_id)?;
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
