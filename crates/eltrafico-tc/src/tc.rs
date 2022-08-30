use std::collections::HashSet;

use crate::utils::ifconfig;
use crate::{run, run_out, Result};

const MIN_RATE: &str = "8";

// "TC store rates as a 32-bit unsigned integer in bps internally, so we can specify a max rate of 4294967295 bps"
// (source: `$ man tc`)
const MAX_RATE: &str = "4294967295";

// This ID seems to be fixed for the ingress QDisc
pub const INGRESS_QDISC_PARENT_ID: &str = "ffff:fff1";

#[derive(Clone)]
pub struct QDisc {
    pub device: String,
    pub id: usize,
    pub root_class_id: usize,
}

//FIXME
fn _clean_up(remove_ifb_device: bool, shutdown_ifb_device: Option<String>) -> Result<()> {
    log::info!("Cleaning up IFB device");
    if remove_ifb_device {
        run!("rmmod ifb")
    } else {
        run!("ip link set dev {shutdown_ifb_device:?} down")
    }
}

fn activate_device(name: &str) -> Result<()> {
    run!("ip link set dev {} up", name)
}

fn create_ifb_device() -> Result<String> {
    let before: HashSet<String> = ifconfig()?.into_iter().map(|i| i.name).collect();
    run!("modprobe ifb numifbs=1")?;
    let after: HashSet<String> = ifconfig()?.into_iter().map(|i| i.name).collect();
    let created_interface_name = after
        .difference(&before)
        .next()
        .expect("Error creating  interface");

    activate_device(created_interface_name)?;
    Ok(created_interface_name.to_string())
}

fn acquire_ifb_device() -> Result<String> {
    let interfaces = ifconfig()?;
    if let Some(interface) = interfaces.iter().find(|i| i.name.starts_with("ifb")) {
        if !interface.is_up() {
            activate_device(&interface.name)?;
            //TODO
            //
            //# Deactivate existing IFB device if it wasn't activated
            //atexit.register(_clean_up, shutdown_ifb_device=device_name)
        }
        Ok(interface.name.to_string())
    } else {
        //TODO
        // # Clean up IFB device if it was created
        // atexit.register(_clean_up, remove_ifb_device=True)
        create_ifb_device()
    }
}

fn find_free_ids(ids: impl Iterator<Item = usize>) -> usize {
    let set: HashSet<_> = ids.collect();
    let mut current = 1;
    while set.contains(&current) {
        current += 1;
    }
    current
}

fn get_free_qdisc_id(device: &str) -> Result<usize> {
    let output = run_out!("tc qdisc show dev {device}")??;

    let mut ids: Vec<usize> = vec![];
    for line in output.lines() {
        if !line.starts_with("qdisc") {
            log::warn!("Failed to parse line: {line}");
            continue;
        }
        if let Some(p) = line.split_whitespace().nth(2) {
            let mut p = p.split(':');
            if let Some(qdisc_id) = p.next() {
                let qdisc_id = match qdisc_id.parse() {
                    Ok(id) => id,
                    Err(_id) => {
                        // This should only happen for the ingress QDisc `qdisc ingress ffff:`
                        usize::from_str_radix(qdisc_id, 16)?
                    }
                };
                ids.push(qdisc_id);
            }
        }
    }
    Ok(find_free_ids(ids.into_iter()))
}

fn get_free_class_id(interface: &str, qdisc_id: usize) -> crate::Result<usize> {
    let output = run_out!("tc class show dev {}", interface)??;
    let mut ids: Vec<usize> = vec![];
    for line in output.lines() {
        if !line.starts_with("class") {
            log::warn!("Failed to parse line: {line}");
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
    Ok(find_free_ids(ids.into_iter()))
}

pub fn tc_setup(
    device: String,
    download_rate: Option<String>,
    download_minimum_rate: Option<String>,
    upload_rate: Option<String>,
    upload_minimum_rate: Option<String>,
    default_download_priority: Option<usize>,
    default_upload_priority: Option<usize>,
) -> Result<(QDisc, QDisc)> {
    // Rust way to mimic python optional
    let download_rate = download_rate.unwrap_or_else(|| MAX_RATE.into());
    let download_minimum_rate = download_minimum_rate.unwrap_or_else(|| MIN_RATE.into());
    let upload_rate = upload_rate.unwrap_or_else(|| MAX_RATE.into());
    let upload_minimum_rate = upload_minimum_rate.unwrap_or_else(|| MIN_RATE.into());
    let default_download_priority = default_download_priority.unwrap_or(0);
    let default_upload_priority = default_upload_priority.unwrap_or(0);

    // set up IFB device
    run!("tc qdisc add dev {device} handle ffff: ingress")?;
    let ifb_device = acquire_ifb_device()?;
    run!(
        "tc filter add dev {device} parent ffff: protocol ip u32 match u32 0 0 action mirred egress redirect dev {ifb_device}"
    )?;

    // Create IFB device QDisc and root class limited at download_rate
    let ifb_device_qdisc_id = get_free_qdisc_id(&ifb_device)?;
    run!("tc qdisc add dev {ifb_device} root handle {ifb_device_qdisc_id}: htb",)?;
    let ifb_device_root_class_id = get_free_class_id(&ifb_device, ifb_device_qdisc_id)?;
    run!(
        "tc class add dev {ifb_device} parent {ifb_device_qdisc_id}: classid {ifb_device_qdisc_id}:{ifb_device_root_class_id} htb rate {download_rate}"
    )?;

    let ingress_qdisc = QDisc {
        device: ifb_device.clone(),
        id: ifb_device_qdisc_id,
        root_class_id: ifb_device_root_class_id,
    };
    // Create default class that all traffic is routed through that doesn't match any other filter
    let ifb_default_class_id = tc_add_htb_class(
        &ingress_qdisc,
        Some(download_rate),
        Some(download_minimum_rate),
        Some(default_download_priority),
    )?;
    run!(
        "tc filter add dev {ifb_device} parent {ifb_device_qdisc_id}: prio 2 protocol ip u32 match u32 0 0 flowid {ifb_device_qdisc_id}:{ifb_default_class_id}"
    )?;

    // Create interface QDisc and root class limited at upload_rate
    let device_qdisc_id = get_free_qdisc_id(&device)?;
    run!("tc qdisc add dev {device} root handle {device_qdisc_id}: htb",)?;
    let device_root_class_id = get_free_class_id(&device, device_qdisc_id)?;

    run!(
        "tc class add dev {device} parent {device_qdisc_id}: classid {device_qdisc_id}:{device_root_class_id} htb rate {upload_rate}"
    )?;

    let egress_qdisc = QDisc {
        device: device.to_string(),
        id: device_qdisc_id,
        root_class_id: device_root_class_id,
    };

    // Create default class that all traffic is routed through that doesn't match any other filter
    let device_default_class_id = tc_add_htb_class(
        &egress_qdisc,
        Some(upload_rate),
        Some(upload_minimum_rate),
        Some(default_upload_priority),
    )?;
    run!(
        "tc filter add dev {device} parent {device_qdisc_id}: prio 2 protocol ip u32 match u32 0 0 flowid {device_qdisc_id}:{device_default_class_id}"
    )?;

    Ok((ingress_qdisc, egress_qdisc))
}

pub fn tc_add_htb_class(
    qdisc: &QDisc,
    ceil: Option<String>,
    rate: Option<String>,
    priority: Option<usize>,
) -> Result<usize> {
    let ceil = ceil.unwrap_or_else(|| MAX_RATE.into());
    let rate = rate.unwrap_or_else(|| MIN_RATE.into());
    let priority = priority.unwrap_or(0);
    let class_id = get_free_class_id(&qdisc.device, qdisc.id)?;
    // rate of 1byte/s is the lowest we can specify. All classes added this way should
    // only be allowed to borrow from the parent class, otherwise it's possible to
    // specify a rate higher than the global rate
    run!(
        "tc class add dev {} parent {}:{} classid {}:{class_id} htb rate {rate} ceil {ceil} prio {priority}"
        ,qdisc.device
        ,qdisc.id
        ,qdisc.root_class_id
        ,qdisc.id
    )?;

    Ok(class_id)
}

fn get_filter_ids(device: &str) -> Result<HashSet<String>> {
    let output = run_out!("tc filter show dev {}", device)??;

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

pub fn tc_add_u32_filter(qdisc: &QDisc, predicate: String, class_id: usize) -> Result<String> {
    let before = get_filter_ids(&qdisc.device)?;
    run!(
        "tc filter add dev {} protocol ip parent {}: prio 1 u32 {predicate} flowid {}:{class_id}",
        qdisc.device,
        qdisc.id,
        qdisc.id,
    )?;
    let after = get_filter_ids(&qdisc.device)?;

    let difference: Vec<_> = after.difference(&before).collect();

    if let Some(diff) = difference.get(0) {
        if difference.len() > 1 {
            log::warn!("Parsed ambiguous filter handle: {:?}", difference);
        }
        Ok(diff.to_string())
    } else {
        panic!("tc_add_u32_filter paniced")
    }
}

pub fn tc_remove_u32_filter(qdisc: &QDisc, filter_id: String) -> Result<()> {
    run!(
        "tc filter del dev {} parent {}: handle {filter_id} prio 1 protocol ip u32",
        qdisc.device,
        qdisc.id,
    )
}

pub fn tc_remove_qdisc(device: String, parent: Option<String>) -> Result<()> {
    run!(
        "tc qdisc del dev {device} parent {}",
        parent.unwrap_or_else(|| "root".into())
    )?;
    Ok(())
}
