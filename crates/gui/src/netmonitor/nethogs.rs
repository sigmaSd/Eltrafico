use crate::gui::UpdateGuiMessage;
use crate::CatchAll;
use glib::Sender;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::process::{Command, Stdio};

pub fn nethogs(tx: Sender<UpdateGuiMessage>) -> CatchAll<()> {
    let mut cmd = Command::new("pkexec")
        .arg("nethogs")
        .arg("-C")
        .arg("-t")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = cmd
        .stdout
        .as_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Err while reading nethogs output"))?;

    let mut stdout = std::io::BufReader::new(stdout);
    let mut raw_output = String::new();

    loop {
        while !raw_output.ends_with("\n\n") {
            stdout.read_line(&mut raw_output)?;

            // nethogs stopped
            if raw_output.is_empty() {
                return Ok(());
            }
        }
        // skip nethogs -t header
        if !raw_output.contains("Refreshing") {
            raw_output.clear();
            continue;
        }
        // parse
        let parsed_data = parse_data(&raw_output);

        // calculate the total download and upload rate
        let global_speed = parsed_data.iter().fold((0., 0.), |mut acc, (_, (u, d))| {
            acc.0 += u;
            acc.1 += d;
            acc
        });

        // send data to gui thread
        tx.send(UpdateGuiMessage::CurrentGlobalSpeed(global_speed))?;
        tx.send(UpdateGuiMessage::CurrentProgramSpeed(parsed_data))?;

        raw_output.clear();
    }
}

fn parse_data(raw_output: &str) -> HashMap<String, (f32, f32)> {
    raw_output
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut fields = l.split_whitespace();
            let exe = fields.next()?;
            let pid = exe.rsplit('/').nth(1)?;
            let name = Command::new("cat")
                .arg(format!("/proc/{}/stat", pid))
                .output()
                .ok()?;
            let mut name = String::from_utf8(name.stdout)
                .ok()?
                .split_whitespace()
                .nth(1)?
                .to_string();
            // remove ( )
            name.remove(0);
            name.pop();

            let up = fields.next()?.parse::<f32>().ok()?.round();
            let down = fields.next()?.parse::<f32>().ok()?.round();
            Some((name, (up, down)))
        })
        .collect()
}
