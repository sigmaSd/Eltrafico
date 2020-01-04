use crate::tc;
use std::collections::HashMap;
pub fn limit(
    programs_to_limit: HashMap<String, (Option<String>, Option<String>)>,
    global_down: Option<String>,
    global_up: Option<String>,
) {
    let mut limits = HashMap::new();

    let (ingress, egress) = tc::tc_setup("wlp3s0", global_down, global_up).unwrap();
    let (ingress_interface, ingress_qdisc_id, ingress_root_class_id) = ingress;
    let (egress_interface, egress_qdisc_id, egress_root_class_id) = egress;

    for (prgoram, (down, up)) in programs_to_limit {
        let ingress_class_id = if let Some(down) = down {
            Some(
                tc::tc_add_htb_class(
                    ingress_interface,
                    ingress_qdisc_id,
                    ingress_root_class_id,
                    down.to_string(),
                )
                .unwrap(),
            )
        } else {
            None
        };

        let egress_class_id = if let Some(up) = up {
            Some(
                tc::tc_add_htb_class(
                    egress_interface,
                    egress_qdisc_id,
                    egress_root_class_id,
                    up.to_string(),
                )
                .unwrap(),
            )
        } else {
            None
        };

        limits.insert(prgoram, (ingress_class_id, egress_class_id));
    }

    let mut filtered_ports: HashMap<(&str, String), String> = HashMap::new();
    loop {
        let active_connections = crate::lsof::lsof().unwrap();
        let mut new_ports = HashMap::new();

        for (program, connections) in active_connections {
            if !limits.contains_key(&program) {
                continue;
            }
            let (ingress_class_id, egress_class_id) = limits[&program];
            for con in connections {
                if let Some(ingress_class_id) = ingress_class_id {
                    let ingress_port = ("ingress", con.lport.clone());
                    if !filtered_ports.contains_key(&ingress_port) {
                        let ingress_filter_id = tc::add_ingress_filter(
                            &con.lport,
                            ingress_interface,
                            ingress_class_id,
                            ingress_qdisc_id,
                        )
                        .unwrap();
                        new_ports.insert(ingress_port, ingress_filter_id);
                    } else {
                        new_ports
                            .insert(ingress_port.clone(), filtered_ports[&ingress_port].clone());
                    }
                }

                if let Some(egress_class_id) = egress_class_id {
                    let egress_port = ("egress", con.lport.clone());
                    if !filtered_ports.contains_key(&egress_port) {
                        let egress_filter_id = tc::add_egress_filter(
                            &con.lport,
                            egress_interface,
                            egress_class_id,
                            egress_qdisc_id,
                        )
                        .unwrap();
                        new_ports.insert(egress_port, egress_filter_id);
                    } else {
                        new_ports.insert(egress_port.clone(), filtered_ports[&egress_port].clone());
                    }
                }
            }
        }
        for (p, id) in filtered_ports {
            if !new_ports.contains_key(&p) {
                match p.0 {
                    "ingress" => {
                        tc::tc_remove_u32_filter(ingress_interface, &id, ingress_qdisc_id);
                    }
                    "egress" => {
                        tc::tc_remove_u32_filter(egress_interface, &id, egress_qdisc_id);
                    }
                    _ => unreachable!(),
                }
            }
        }

        filtered_ports = new_ports;
    }
}
