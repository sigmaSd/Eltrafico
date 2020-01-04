// "TC store rates as a 32-bit unsigned integer in bps internally, so we can specify a max rate of 4294967295 bps"
// (source: `$ man tc`)
const MAX_RATE: &str = "4294967295";

// This ID seems to be fixed for the ingress QDisc
const INGRESS_QDISC_PARENT_ID: &str = "ffff:fff1";

use std::process::*;
pub type CatchAll<T> = Result<T, Box<dyn std::error::Error>>;
type Gress = (&'static str, usize, usize);

pub fn tc_setup(
    interface: &'static str,
    download_rate: Option<String>,
    upload_rate: Option<String>,
) -> CatchAll<(Gress, Gress)> {
    let download_rate = download_rate.unwrap_or_else(|| MAX_RATE.to_string());
    let upload_rate = upload_rate.unwrap_or_else(|| MAX_RATE.to_string());

    // set up IFB device
    run(format!(
        "tc qdisc add dev {} handle ffff: ingress",
        interface
    ))?;
    let ifb_device = acquire_ifb_device()?;

    run(format!(
        "tc filter add dev {} parent ffff: protocol ip u32 match u32 0 0 action mirred egress redirect dev {}",
        interface, ifb_device
    ))?;

    // Create IFB device QDisc and root class limited at download_rate
    let ifb_device_qdisc_id = get_free_qdisc_id(ifb_device);
    run(format!(
        "tc qdisc add dev {} root handle {}: htb",
        ifb_device, ifb_device_qdisc_id
    ))?;
    let ifb_device_root_class_id = get_free_class_id(ifb_device, ifb_device_qdisc_id)?;
    run(format!(
        "tc class add dev {} parent {}: classid {}:{} htb rate {}",
        ifb_device,
        ifb_device_qdisc_id,
        ifb_device_qdisc_id,
        ifb_device_root_class_id,
        download_rate
    ))?;

    // Create default class that all traffic is routed through that doesn't match any other filter
    let ifb_default_class_id = tc_add_htb_class(
        ifb_device,
        ifb_device_qdisc_id,
        ifb_device_root_class_id,
        download_rate,
    )?;
    run(format!(
        "tc filter add dev {} parent {}: prio 2 protocol ip u32 match u32 0 0 flowid {}:{}",
        ifb_device, ifb_device_qdisc_id, ifb_device_qdisc_id, ifb_default_class_id
    ))?;

    // Create interface QDisc and root class limited at upload_rate
    let interface_qdisc_id = get_free_qdisc_id(interface);
    run(format!(
        "tc qdisc add dev {} root handle {}: htb",
        interface, interface_qdisc_id
    ))?;
    let interface_root_class_id = get_free_class_id(interface, interface_qdisc_id)?;
    run(format!(
        "tc class add dev {} parent {}: classid {}:{} htb rate {}",
        interface, interface_qdisc_id, interface_qdisc_id, interface_root_class_id, upload_rate
    ))?;

    // Create default class that all traffic is routed through that doesn't match any other filter
    let interface_default_class_id = tc_add_htb_class(
        interface,
        interface_qdisc_id,
        interface_root_class_id,
        upload_rate,
    )?;
    run(format!(
        "tc filter add dev {} parent {}: prio 2 protocol ip u32 match u32 0 0 flowid {}:{}",
        interface, interface_qdisc_id, interface_qdisc_id, interface_default_class_id
    ))?;

    Ok((("ifb0", 1, 1), ("wlp3s0", 1, 1)))
}

pub fn tc_add_htb_class(
    interface: &'static str,
    parent_qdisc_id: usize,
    parent_class_id: usize,
    rate: String,
) -> CatchAll<usize> {
    let class_id = get_free_class_id(interface, parent_qdisc_id)?;
    // rate of 1byte/s is the lowest we can specify. All classes added this way should only be allowed to borrow from the
    // parent class, otherwise it's possible to specify a rate higher than the global rate
    run(format!(
        "tc class add dev {} parent {}:{} classid {}:{} htb rate 8 ceil {}",
        interface, parent_qdisc_id, parent_class_id, parent_qdisc_id, class_id, rate
    ))?;

    Ok(class_id)
}

fn get_free_qdisc_id(_ifb_device: &'static str) -> usize {
    1
}

fn get_free_class_id(interface: &'static str, qdisc_id: usize) -> CatchAll<usize> {
    let output = run(format!("tc class show dev {}", interface))?;
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

fn acquire_ifb_device() -> CatchAll<&'static str> {
    //TODO some stuff goes here
    create_ifb_device()
}

fn create_ifb_device() -> CatchAll<&'static str> {
    let name = "ifb0";
    activate_interface(name)?;
    Ok(name)
}

fn activate_interface(name: &'static str) -> CatchAll<()> {
    run(format!("ip link set dev {} up", name))?;
    Ok(())
}

fn run(v: String) -> CatchAll<Output> {
    let v = v.split_whitespace();
    Ok(Command::new("sudo")
        .args(v.collect::<Vec<&str>>())
        .output()?)
}

#[test]
fn clean() {
    clean_up("ifb0", "wlp3s0");
}

pub fn clean_up(ingress_interface: &'static str, egress_interface: &'static str) -> CatchAll<()> {
    tc_remove_qdisc(ingress_interface, None)?;
    tc_remove_qdisc(egress_interface, None)?;
    tc_remove_qdisc(egress_interface, Some(INGRESS_QDISC_PARENT_ID))?;
    Ok(())
}
fn tc_remove_qdisc(interface: &'static str, parent: Option<&'static str>) -> CatchAll<()> {
    run(format!(
        "tc qdisc del dev {} parent {}",
        interface,
        parent.unwrap_or("root")
    ))?;
    Ok(())
}

pub fn add_egress_filter(
    port: &str,
    egress_interface: &str,
    egress_class_id: usize,
    egress_qdisc_id: usize,
) -> CatchAll<String> {
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
    ingress_class_id: usize,
    ingress_qdisc_id: usize,
) -> CatchAll<String> {
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
) -> CatchAll<String> {
    let before = get_filter_ids(interface)?;
    run(format!(
        "tc filter add dev {} protocol ip parent {}: prio 1 u32 {} flowid {}:{}",
        interface, parent_qdisc_id, predicate, parent_qdisc_id, class_id
    ))?;
    let after = get_filter_ids(interface)?;

    let mut difference = after.difference(&before);

    if let Some(diff) = difference.next() {
        Ok(diff.to_string())
    } else {
        // ctrlc was maybe used
        std::process::exit(0);
        //panic!("tc_add_u32_filter paniced")
    }
}

use std::collections::HashSet;
fn get_filter_ids(interface: &str) -> CatchAll<HashSet<String>> {
    let output = run(format!("tc filter show dev {}", interface))?;
    let output = String::from_utf8(output.stdout)?;

    let mut ids = HashSet::new();
    for line in output.lines() {
        if !line.starts_with("filter") {
            continue;
        }
        if let Some(hit) = line.split_whitespace().nth(11) {
            ids.insert(hit.to_string());
        }
    }
    Ok(ids)
}

pub fn tc_remove_u32_filter(interface: &str, filter_id: &str, parent_qdisc_id: usize) {
    run(format!(
        "tc filter del dev {} parent {}: handle {} prio 1 protocol ip u32",
        interface, parent_qdisc_id, filter_id
    ))
    .unwrap();
}
