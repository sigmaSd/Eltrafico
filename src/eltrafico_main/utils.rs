use crate::CatchAll;
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
    //dbg!(&v);

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

pub fn check_for_dependencies(dependencies: &[&str]) -> Result<(), String> {
    for tool in dependencies {
        if let Err(e) = std::process::Command::new(tool)
            // use -h so programs like nethogs dont stay open indefinitely
            .arg("-h")
            .output()
        {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Err(format!("Missing program: {}", tool));
            }
        }
    }
    Ok(())
}

#[test]
fn tifconfig() {
    dbg!(ifconfig());
}
// ifconfig
pub fn ifconfig() -> CatchAll<Vec<Interface>> {
    let output = run!("ifconfig -a")?;
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
            } else {
                Status::Down
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

#[derive(PartialEq, Eq, Debug)]
enum Status {
    Up,
    Down,
}

pub fn finde_eltrafico_tc() -> String {
    if check_for_dependencies(&["eltrafico_tc"]).is_err() {
        const ERR_MSG: &str =
            "Could not find eltrafico_tc, you can specify its path with --eltrafico-tc";
        let args: Vec<String> = std::env::args().collect();
        let pos = args
            .iter()
            .position(|a| a.as_str() == "--eltrafico-tc")
            .expect(ERR_MSG);
        let path = args.get(pos + 1).expect(ERR_MSG);
        //pkexec require absolute path
        let path = std::path::Path::new(path).canonicalize().unwrap();
        if !path.exists() {
            panic!("Can't find {:?}", path);
        }
        path.to_str().unwrap().to_string()
    } else {
        "eltrafico_tc".into()
    }
}
