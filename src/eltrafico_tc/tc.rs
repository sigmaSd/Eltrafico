use crate::run;
use crate::utils::ifconfig;
use std::collections::HashSet;
// "TC store rates as a 32-bit unsigned integer in bps internally, so we can specify a max rate of 4294967295 bps"
// (source: `$ man tc`)
const MAX_RATE: &str = "4294967295";

// This ID seems to be fixed for the ingress QDisc
const INGRESS_QDISC_PARENT_ID: &str = "ffff:fff1";

pub struct Traffic {
    pub interface: String,
    pub qdisc_id: usize,
    pub class_id: usize,
}
impl Traffic {
    fn new(interface: String, qdisc_id: usize, class_id: usize) -> Self {
        Traffic {
            interface,
            qdisc_id,
            class_id,
        }
    }
}

pub fn tc_setup(
    interface: &str,
    download_rate: Option<String>,
    upload_rate: Option<String>,
) -> crate::CatchAll<(Traffic, Traffic)> {
    let download_rate = download_rate.unwrap_or_else(|| MAX_RATE.to_string());
    let upload_rate = upload_rate.unwrap_or_else(|| MAX_RATE.to_string());

    // set up IFB device
    run!("tc qdisc add dev {} handle ffff: ingress", interface)?;
    let ifb_device = acquire_ifb_device()?;

    run!(
        "tc filter add dev {} parent ffff: protocol ip u32 match u32 0 0 action mirred egress redirect dev {}",
        interface, ifb_device
    )?;

    // Create IFB device QDisc and root class limited at download_rate
    let ifb_device_qdisc_id = get_free_qdisc_id(&ifb_device)?;
    run!(
        "tc qdisc add dev {} root handle {}: htb",
        ifb_device,
        ifb_device_qdisc_id
    )?;
    let ifb_device_root_class_id = get_free_class_id(&ifb_device, ifb_device_qdisc_id)?;
    run!(
        "tc class add dev {} parent {}: classid {}:{} htb rate {}",
        ifb_device,
        ifb_device_qdisc_id,
        ifb_device_qdisc_id,
        ifb_device_root_class_id,
        download_rate
    )?;

    // assemble ingress traffic
    let ingress = Traffic::new(ifb_device, ifb_device_qdisc_id, ifb_device_root_class_id);

    // Create default class that all traffic is routed through that doesn't match any other filter
    let ifb_default_class_id = tc_add_htb_class(&ingress, &download_rate)?;
    run!(
        "tc filter add dev {} parent {}: prio 2 protocol ip u32 match u32 0 0 flowid {}:{}",
        ingress.interface,
        ingress.qdisc_id,
        ingress.qdisc_id,
        ifb_default_class_id
    )?;

    // Create interface QDisc and root class limited at upload_rate
    let interface_qdisc_id = get_free_qdisc_id(interface)?;
    run!(
        "tc qdisc add dev {} root handle {}: htb",
        interface,
        interface_qdisc_id
    )?;
    let interface_root_class_id = get_free_class_id(interface, interface_qdisc_id)?;
    run!(
        "tc class add dev {} parent {}: classid {}:{} htb rate {}",
        interface,
        interface_qdisc_id,
        interface_qdisc_id,
        interface_root_class_id,
        upload_rate
    )?;

    // assemble egress traffic
    let egress = Traffic::new(
        interface.to_string(),
        interface_qdisc_id,
        interface_root_class_id,
    );

    // Create default class that all traffic is routed through that doesn't match any other filter
    let interface_default_class_id = tc_add_htb_class(&egress, &upload_rate)?;
    run!(
        "tc filter add dev {} parent {}: prio 2 protocol ip u32 match u32 0 0 flowid {}:{}",
        egress.interface,
        egress.qdisc_id,
        egress.qdisc_id,
        interface_default_class_id
    )?;

    Ok((ingress, egress))
}

pub fn tc_add_htb_class(parent_traffic: &Traffic, rate: &str) -> crate::CatchAll<usize> {
    let class_id = get_free_class_id(&parent_traffic.interface, parent_traffic.qdisc_id)?;
    // rate of 1byte/s is the lowest we can specify. All classes added this way should only be allowed to borrow from the
    // parent class, otherwise it's possible to specify a rate higher than the global rate
    run!(
        "tc class add dev {} parent {}:{} classid {}:{} htb rate 8 ceil {}",
        parent_traffic.interface,
        parent_traffic.qdisc_id,
        parent_traffic.class_id,
        parent_traffic.qdisc_id,
        class_id,
        rate
    )?;

    Ok(class_id)
}

fn get_free_qdisc_id(interface: &str) -> crate::CatchAll<usize> {
    let output = run!("tc qdisc show dev {}", interface)?;
    let output = String::from_utf8(output.stdout)?;
    let mut ids: Vec<usize> = vec![];
    for line in output.lines() {
        if !line.starts_with("qdisc") {
            continue;
        }
        if let Some(p) = line.split_whitespace().nth(2) {
            let mut p = p.split(':');
            if let Some(qdisc_id) = p.next() {
                let qdisc_id = match qdisc_id.parse() {
                    Ok(id) => id,
                    // This should only happen for the ingress QDisc `qdisc ingress ffff:`
                    Err(_id) => continue,
                };
                ids.push(qdisc_id);
            }
        }
    }
    Ok(ids.into_iter().max().unwrap_or(0) + 1)
}

fn get_free_class_id(interface: &str, qdisc_id: usize) -> crate::CatchAll<usize> {
    let output = run!("tc class show dev {}", interface)?;
    let output = String::from_utf8(output.stdout)?;
    let mut ids: Vec<usize> = vec![];
    for line in output.lines() {
        if !line.starts_with("class") {
            continue;
        }
        if let Some(p) = line.split_whitespace().nth(2) {
            let mut p = p.split(':');
            let current_qdisc_id = p.next();
            if let Some(current_qdisc_id) = current_qdisc_id {
                if current_qdisc_id.parse::<usize>()? == qdisc_id {
                    if let Some(class_id) = p.next() {
                        ids.push(class_id.parse()?);
                    }
                }
            }
        }
    }
    Ok(ids.into_iter().max().unwrap_or(0) + 1)
}

pub fn acquire_ifb_device() -> crate::CatchAll<String> {
    let interfaces = ifconfig()?;
    if let Some(interface) = interfaces.iter().find(|i| i.name.starts_with("ifb")) {
        if !interface.is_up() {
            activate_interface(&interface.name)?;
        }
        Ok(interface.name.to_string())
    } else {
        create_ifb_device()
    }
}

fn create_ifb_device() -> crate::CatchAll<String> {
    let before: HashSet<String> = ifconfig()?.into_iter().map(|i| i.name).collect();
    run!("modprobe ifb numifbs=1")?;
    let after: HashSet<String> = ifconfig()?.into_iter().map(|i| i.name).collect();
    let mut created_interface_name: Vec<&String> = after.difference(&before).collect();
    let created_interface_name = created_interface_name
        .pop()
        .expect("Error creating  interface");
    activate_interface(created_interface_name)?;
    Ok(created_interface_name.to_string())
}

fn activate_interface(name: &str) -> crate::CatchAll<()> {
    run!("ip link set dev {} up", name)?;
    Ok(())
}

#[test]
fn clean() {
    clean_up("ifb0", "wlp3s0").unwrap();
}

pub fn clean_up(ingress_interface: &str, egress_interface: &str) -> crate::CatchAll<()> {
    tc_remove_qdisc(ingress_interface, None)?;
    tc_remove_qdisc(egress_interface, None)?;
    tc_remove_qdisc(egress_interface, Some(INGRESS_QDISC_PARENT_ID))?;
    Ok(())
}
fn tc_remove_qdisc(interface: &str, parent: Option<&str>) -> crate::CatchAll<()> {
    run!(
        "tc qdisc del dev {} parent {}",
        interface,
        parent.unwrap_or("root")
    )?;
    Ok(())
}

pub fn add_egress_filter(
    port: &str,
    egress_interface: &str,
    egress_qdisc_id: usize,
    egress_class_id: usize,
) -> crate::CatchAll<String> {
    let filter_id = tc_add_u32_filter(
        egress_interface,
        &format!("match ip sport {} 0xffff", port),
        egress_qdisc_id,
        egress_class_id,
    )?;
    Ok(filter_id)
}

pub fn add_ingress_filter(
    port: &str,
    ingress_interface: &str,
    ingress_qdisc_id: usize,
    ingress_class_id: usize,
) -> crate::CatchAll<String> {
    let filter_id = tc_add_u32_filter(
        ingress_interface,
        &format!("match ip dport {} 0xffff", port),
        ingress_qdisc_id,
        ingress_class_id,
    )?;
    Ok(filter_id)
}

fn tc_add_u32_filter(
    interface: &str,
    predicate: &str,
    parent_qdisc_id: usize,
    class_id: usize,
) -> crate::CatchAll<String> {
    let before = get_filter_ids(interface)?;
    run!(
        "tc filter add dev {} protocol ip parent {}: prio 1 u32 {} flowid {}:{}",
        interface,
        parent_qdisc_id,
        predicate,
        parent_qdisc_id,
        class_id
    )?;
    let after = get_filter_ids(interface)?;

    let mut difference = after.difference(&before);

    if let Some(diff) = difference.next() {
        Ok(diff.to_string())
    } else {
        panic!("tc_add_u32_filter paniced")
    }
}

fn get_filter_ids(interface: &str) -> crate::CatchAll<HashSet<String>> {
    let output = run!("tc filter show dev {}", interface)?;
    let output = String::from_utf8(output.stdout)?;

    let mut ids = HashSet::new();
    for line in output.lines() {
        if !line.starts_with("filter") {
            continue;
        }
        if let Some(hit) = line.split_whitespace().nth(11) {
            // regex ([a-z0-9]+::[a-z0-9]+?)
            if hit.split("::").count() == 2 {
                ids.insert(hit.to_string());
            }
        }
    }
    Ok(ids)
}

pub fn tc_remove_u32_filter(
    interface: &str,
    filter_id: &str,
    parent_qdisc_id: usize,
) -> crate::CatchAll<()> {
    run!(
        "tc filter del dev {} parent {}: handle {} prio 1 protocol ip u32",
        interface,
        parent_qdisc_id,
        filter_id
    )?;
    Ok(())
}

#[test]
fn trun() {
    dbg!(run!("lsof -i -n").unwrap());
}
