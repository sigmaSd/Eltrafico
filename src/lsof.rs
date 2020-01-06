use crate::CatchAll;
use std::collections::HashMap;
use std::process::*;

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
