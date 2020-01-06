use crate::tc;
use glib::Sender;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
type Rate = Option<String>;
pub fn limit(
    _programs_to_limit: Arc<Mutex<HashMap<String, (Rate, Rate)>>>,
    interface: &str,
    delay: Option<usize>,
    running: Arc<AtomicBool>,
    tx: Sender<String>,
    rx: mpsc::Receiver<(String, (Rate, Rate))>,
) -> crate::CatchAll<()> {
    use TrafficType::*;

    let program_to_trafficid_map = Rc::new(RefCell::new(HashMap::new()));
    let program_to_trafficid_map_c = program_to_trafficid_map.clone();

    // lock the programs_to_limit map
    //let mut programs_to_limit = programs_to_limit.lock().unwrap();
    // look for a specified global limit
    // let global_limit = if let Some(limit) = (*programs_to_limit).remove("global") {
    //     limit
    // } else {
    //     (None, None)
    // };

    let (ingress, egress) = tc::tc_setup(interface, None, None)?;
    let (ingress_interface, ingress_qdisc_id, ingress_root_class_id) = ingress;
    let (egress_interface, egress_qdisc_id, egress_root_class_id) = egress;

    let egress_interface_c = egress_interface.clone();
    let ingress_interface_c = ingress_interface.clone();

    if let Ok(programs_to_limit) = rx.try_recv() {
        dbg!(4);
        let (program, (down, up)) = programs_to_limit;
        let ingress_class_id = if let Some(down) = down {
            Some(
                tc::tc_add_htb_class(
                    &ingress_interface_c,
                    ingress_qdisc_id,
                    ingress_root_class_id,
                    &down,
                )
                .unwrap(),
            )
        } else {
            None
        };

        let egress_class_id = if let Some(up) = up {
            Some(
                tc::tc_add_htb_class(
                    &egress_interface_c,
                    egress_qdisc_id,
                    egress_root_class_id,
                    &up,
                )
                .unwrap(),
            )
        } else {
            None
        };

        program_to_trafficid_map_c
            .borrow_mut()
            .insert(program, (ingress_class_id, egress_class_id));
    };

    let mut filtered_ports: HashMap<(TrafficType, String), String> = HashMap::new();

    while running.load(Ordering::SeqCst) {
        let active_connections = crate::lsof::lsof()?;
        let mut active_ports = HashMap::new();
        if let Ok(programs_to_limit) = rx.try_recv() {
            let (program, (down, up)) = programs_to_limit;
            let ingress_class_id = if let Some(down) = down {
                Some(
                    tc::tc_add_htb_class(
                        &ingress_interface_c,
                        ingress_qdisc_id,
                        ingress_root_class_id,
                        &down,
                    )
                    .unwrap(),
                )
            } else {
                None
            };

            let egress_class_id = if let Some(up) = up {
                Some(
                    tc::tc_add_htb_class(
                        &egress_interface_c,
                        egress_qdisc_id,
                        egress_root_class_id,
                        &up,
                    )
                    .unwrap(),
                )
            } else {
                None
            };

            program_to_trafficid_map_c
                .borrow_mut()
                .insert(program.clone(), (ingress_class_id, egress_class_id));
        };

        // look for new ports to filter
        for (program, connections) in active_connections {
            // if !(*programs_to_limit).contains_key(&program) {
            //     // this is a new program
            //     // send its name to the gui
            //     tx.send(program.clone())?;
            //     //(*programs_to_limit).insert(program.clone(), (None, None));
            // }
            let program_in_map = program_to_trafficid_map
                .borrow()
                .get(&program)
                .map(ToOwned::to_owned);
            let (ingress_class_id, egress_class_id) = match program_in_map {
                Some(id) => id,
                None => {
                    program_to_trafficid_map
                        .borrow_mut()
                        .insert(program.clone(), (None, None));
                    tx.send(program.clone()).unwrap();
                    continue;
                }
            };

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

    Ok(())
}

#[derive(PartialEq, Eq, Hash, Clone)]
enum TrafficType {
    /// Incomming traffic
    Ingress,
    /// Outgoing traffic
    Egress,
}
