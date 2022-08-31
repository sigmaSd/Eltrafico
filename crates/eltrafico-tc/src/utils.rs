use crate::Result;
use std::collections::HashMap;
use std::process::{Command, Output};

#[macro_export]
macro_rules! run {
// run macro
    ($($arg:tt)*) => {{
        let out = $crate::run_out!($($arg)*);
        out.map(|_|())
    }}
}

#[macro_export]
macro_rules! run_out {
    ($($arg:tt)*) => {{
        let out = $crate::utils::run(format!($($arg)*));
        out.map(|v|String::from_utf8(v.stdout))
    }}
}

pub fn run(v: String) -> Result<Output> {
    let cmd = v.clone();
    let mut cmd = cmd.split_whitespace();
    let output = Command::new(cmd.next().expect("Tried to run an empty command"))
        .args(cmd.collect::<Vec<&str>>())
        .output()?;
    if !output.stderr.is_empty() {
        log::error!(
            "Error while running cmd: {:?}\nerr: {}",
            v,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output)
}

#[test]
fn tifconfig() {
    dbg!(ifconfig().unwrap());
}
// ifconfig
pub fn ifconfig() -> Result<Vec<Interface>> {
    let raw_data = std::fs::read_to_string("/proc/net/dev")?;

    //TODO: actually parse statue
    raw_data
        .lines()
        .skip(2)
        .filter_map(|l| l.split(':').next())
        .map(|name| {
            Ok(Interface {
                name: name.trim().to_string(),
                status: Status::Down,
            })
        })
        .collect()
}

#[derive(PartialEq, Eq, Debug)]
pub struct Interface {
    pub name: String,
    status: Status,
}

impl Interface {
    pub fn is_up(&self) -> bool {
        self.status == Status::Up
    }
}

#[derive(PartialEq, Eq, Debug)]
enum Status {
    Up,
    Down,
}

pub fn ss() -> Result<HashMap<String, Vec<Connection>>> {
    let raw_net_table = run_out!("ss -n -t -p  state established")??;

    let mut net_table = HashMap::new();
    for row in raw_net_table.lines().skip(1) {
        let _ = ss_parse(row, &mut net_table);
    }

    Ok(net_table)
}
fn ss_parse(row: &str, net_table: &mut HashMap<String, Vec<Connection>>) -> Option<()> {
    let is_ipv6 =
        |addr: &str| matches!(&addr[0..1], "[") && matches!(&addr[addr.len() - 1..addr.len()], "]");
    let mut row = row.split_whitespace();
    let laddr_lport = row.nth(2)?;
    let raddr_rport = row.next()?;
    let process = row.next()?;

    let mut laddr_lport = laddr_lport.rsplitn(2, ':');
    let lport = laddr_lport.next()?;
    let mut laddr = laddr_lport.next()?.to_string();
    if is_ipv6(&laddr) {
        laddr = laddr[1..laddr.len() - 1].to_string();
    }

    let mut raddr_rport = raddr_rport.rsplitn(2, ':');
    let rport = raddr_rport.next()?;
    let mut raddr = raddr_rport.next()?.to_string();
    if is_ipv6(&raddr) {
        raddr = raddr[1..raddr.len() - 1].to_string();
    }

    let process = process.split('\"').nth(1)?.split('\"').next()?;
    let net_entry: &mut Vec<Connection> = net_table
        .entry(process.to_string())
        .or_insert_with(Vec::new);
    net_entry.push(Connection {
        laddr,
        lport: lport.parse().ok()?,
        raddr,
        rport: rport.parse().ok()?,
    });

    Some(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct Connection {
    pub laddr: String,
    pub lport: usize,
    pub raddr: String,
    pub rport: usize,
}

#[test]
fn test_ss_parse() {
    let row = r#"0              0                        192.168.1.1:5123                     200.2000.200.1111:443            users:(("firefox",pid=1996,fd=128))"#;
    {
        let mut process = HashMap::new();
        ss_parse(row, &mut process);
        assert_eq!(
            process,
            [(
                "firefox".to_string(),
                vec!(Connection {
                    laddr: "192.168.1.1".into(),
                    lport: 5123,
                    raddr: "200.2000.200.1111".into(),
                    rport: 443,
                })
            )]
            .into_iter()
            .collect()
        )
    }
    let two_rows_ipv6 = r#"0      0                        [::1]:9100                                 [::2]:33586               users:(("node_exporter",pid=111305,fd=5))
0      0                        [::1]:33586                                [::1]:9100                users:(("sshd",pid=261247,fd=10))
"#;
    {
        let mut process = HashMap::new();
        two_rows_ipv6.split("\n").for_each(|row| {
            ss_parse(row, &mut process);
        });
        assert_eq!(
            process,
            [
                (
                    "node_exporter".to_string(),
                    vec!(Connection {
                        laddr: "::1".into(),
                        lport: 9100,
                        raddr: "::2".into(),
                        rport: 33586,
                    })
                ),
                (
                    "sshd".to_string(),
                    vec!(Connection {
                        laddr: "::1".into(),
                        lport: 33586,
                        raddr: "::1".into(),
                        rport: 9100,
                    })
                )
            ]
            .into_iter()
            .collect()
        )
    }
}
