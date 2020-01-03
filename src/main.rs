mod lsof;
mod tc;

fn main() -> tc::CatchAll<()> {
    let (ingress, egress) = tc::tc_setup("wlp3s0", Some(200), Some(200))?;
    dbg!(ingress, egress);
    let (ingress_interface, ingress_qdisc_id, ingress_root_class_id) = ingress;
    let (egress_interface, egress_qdisc_id, _egress_root_class_id) = egress;

    let download_rate = 200;
    let egress_class_id = tc::tc_add_htb_class(
        ingress_interface,
        ingress_qdisc_id,
        ingress_root_class_id,
        download_rate,
    )
    .unwrap();

    let active_connections = lsof::lsof().unwrap();
    tc::add_egress_filter(
        &active_connections["firefox"][0].lport,
        egress_interface,
        egress_class_id,
        egress_qdisc_id,
    )
    .unwrap();

    tc::clean_up(ingress_interface, egress_interface)?;
    Ok(())
}
