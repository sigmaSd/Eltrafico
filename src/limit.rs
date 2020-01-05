use crate::tc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
pub fn limit(
    programs_to_limit: HashMap<String, (Option<String>, Option<String>)>,
    global_down: Option<String>,
    global_up: Option<String>,
    interface: &str,
    delay: Option<usize>,
    running: Arc<AtomicBool>,
) -> crate::CatchAll<()> {
    use TrafficType::*;

    let mut program_to_trafficid_map = HashMap::new();

    let (ingress, egress) = tc::tc_setup(interface, global_down, global_up)?;
    let (ingress_interface, ingress_qdisc_id, ingress_root_class_id) = ingress;
    let (egress_interface, egress_qdisc_id, egress_root_class_id) = egress;

    for (prgoram, (down, up)) in programs_to_limit {
        let ingress_class_id = if let Some(down) = down {
            Some(tc::tc_add_htb_class(
                &ingress_interface,
                ingress_qdisc_id,
                ingress_root_class_id,
                &down,
            )?)
        } else {
            None
        };

        let egress_class_id = if let Some(up) = up {
            Some(tc::tc_add_htb_class(
                &egress_interface,
                egress_qdisc_id,
                egress_root_class_id,
                &up,
            )?)
        } else {
            None
        };

        program_to_trafficid_map.insert(prgoram, (ingress_class_id, egress_class_id));
    }

    let mut filtered_ports: HashMap<(TrafficType, String), String> = HashMap::new();

    while running.load(Ordering::SeqCst) {
        let active_connections = crate::lsof::lsof()?;
        let mut active_ports = HashMap::new();

        // look for new ports to filter
        for (program, connections) in active_connections {
            let (ingress_class_id, egress_class_id) = match program_to_trafficid_map.get(&program) {
                Some(id) => id,
                None => continue,
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
                            *ingress_class_id,
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
                            *egress_class_id,
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
