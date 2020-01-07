use crate::CatchAll;
use std::collections::HashMap;
use std::process::{Command, Output};

// run macro
#[macro_export]
macro_rules! run {
    ($($arg:tt)*) => {
        crate::utils::run(format!($($arg)*))
    }
}

pub fn run(v: String) -> CatchAll<Output> {
    // log all cmds
    // dbg!(&v);
    let cmd = v.clone();
    let mut cmd = cmd.split_whitespace();
    let output = Command::new(cmd.next().expect("Tried to run an empty command"))
        .args(cmd.collect::<Vec<&str>>())
        .output()?;
    if !output.stderr.is_empty() {
        eprintln!(
            "Error while running cmd: {:?}\nerr: {}",
            v,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output)
}

// ifconfig
pub fn ifconfig() -> CatchAll<Vec<Interface>> {
    let output = run!("ifconfig")?;
    let output = String::from_utf8(output.stdout)?;

    // get the first line of each paragraph of the output then parse it
    let output: Vec<&str> = output.lines().collect();
    let interfaces = output
        // split by paragraph
        .split(|l| l.is_empty())
        // get the first line of each paragraph
        .filter_map(|p| p.iter().next())
        // parse the interface name and status
        .filter_map(|row| {
            let status = if row.contains("UP") {
                Status::Up
            } else if row.contains("DOWN") {
                Status::Down
            } else {
                return None;
            };
            let name = match row.split(':').next() {
                Some(name) => name,
                None => return None,
            };
            Some(Interface {
                name: name.to_string(),
                status,
            })
        })
        .collect();

    Ok(interfaces)
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

#[test]
fn tifconfig() {
    dbg!(ifconfig());
}

// lsof
#[test]
fn tlsof() {
    dbg!(lsof());
}

pub fn lsof() -> CatchAll<HashMap<String, Vec<Connection>>> {
    let mut net_table = HashMap::new();
    let raw_net_table = String::from_utf8(
        Command::new("lsof")
            .args(&["-i", "-n", "-P"])
            .output()?
            .stdout,
    )?;

    let mut parse_row = |row: &str| -> Option<()> {
        if !row.contains("ESTABLISHED") {
            return None;
        }
        let mut row = row.split_whitespace();

        let name = row.next()?;

        let raw_connection = row.nth(7)?;
        let mut raw_connection = raw_connection.split("->");

        let mut lconn = raw_connection.next()?.split(':');
        let laddr = lconn.next()?;
        let lport = lconn.next()?;

        let mut rconn = raw_connection.next()?.split(':');
        let raddr = rconn.next()?;
        let rport = rconn.next()?;

        let net_entry: &mut Vec<Connection> =
            net_table.entry(name.to_string()).or_insert_with(Vec::new);
        net_entry.push(Connection::new(laddr, lport, raddr, rport));

        Some(())
    };

    for row in raw_net_table.lines().skip(1) {
        let _ = parse_row(row);
    }

    Ok(net_table)
}

#[derive(Debug)]
pub struct Connection {
    laddr: String,
    pub lport: String,
    raddr: String,
    rport: String,
}

impl Connection {
    fn new(laddr: &str, lport: &str, raddr: &str, rport: &str) -> Connection {
        Connection {
            laddr: laddr.to_string(),
            lport: lport.to_string(),
            raddr: raddr.to_string(),
            rport: rport.to_string(),
        }
    }
}
