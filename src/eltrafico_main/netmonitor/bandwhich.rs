use crate::gui::UpdateGuiMessage;
use crate::CatchAll;
use glib::Sender;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::process::{Command, Stdio};

pub fn bandwhich(tx: Sender<UpdateGuiMessage>) -> CatchAll<()> {
    let mut cmd = Command::new("pkexec")
        .arg("bandwhich")
        .arg("-p")
        .arg("--raw")
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

            // bandwhich stopped
            if raw_output.is_empty() {
                return Ok(());
            }
        }
        // skip header
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
            let mut exe = fields.nth(2)?.to_string();
            // remove ""
            exe.remove(0);
            exe.pop();

            let mut up_down = fields.nth(2)?.split('/');
            let up = up_down.next()?.parse::<f32>().ok()? / 1000.;
            let down = up_down.next()?.parse::<f32>().ok()? / 1000.;
            Some((exe, (up, down)))
        })
        .collect()
}

#[derive(Debug)]
pub struct ProgramCurrentSpeed {
    name: String,
    up: f32,
    down: f32,
}

#[test]
fn t_bandwhich() {
    gtk::init().unwrap();

    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    bandwhich(tx).unwrap();

    rx.attach(None, move |message| {
        dbg!(message);
        glib::Continue(true)
    });
    gtk::main();
}
