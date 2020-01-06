use crate::CatchAll;
use std::process::{Command, Output};

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
