#[derive(Eq, PartialEq, Debug, Clone, Default)]
pub struct LimitConfig {
    pub download_rate: Option<String>,
    pub download_minimum_rate: Option<String>,
    pub upload_rate: Option<String>,
    pub upload_minimum_rate: Option<String>,
    pub download_priority: Option<usize>,
    pub upload_priority: Option<usize>,
}

#[derive(Eq, PartialEq, Debug)]
pub enum Message {
    Stop,
    Interface(String),
    Global { config: LimitConfig },
    Program { name: String, config: LimitConfig },
}

impl TryFrom<String> for Message {
    type Error = String;
    fn try_from(msg: String) -> std::result::Result<Self, Self::Error> {
        let parse = || -> Option<Message> {
            let parse_part = |part: Option<&str>| {
                let part = part.map(ToString::to_string);
                if part == Some("None".into()) {
                    None
                } else {
                    part
                }
            };
            use Message::*;
            match msg.trim() {
                "Stop" => Some(Stop),
                msg if msg.starts_with("Interface: ") => {
                    Some(Interface(msg.split("Interface: ").nth(1)?.to_string()))
                }
                msg if msg.starts_with("Global: ") => {
                    let mut msg = msg.split("Global: ").nth(1)?.split_whitespace();
                    let download_rate = parse_part(msg.next());
                    let upload_rate = parse_part(msg.next());
                    let download_minimum_rate = parse_part(msg.next());
                    let upload_minimum_rate = parse_part(msg.next());
                    let download_priority = parse_part(msg.next());
                    let upload_priority = parse_part(msg.next());
                    Some(Global {
                        config: LimitConfig {
                            download_rate,
                            download_minimum_rate,
                            upload_rate,
                            upload_minimum_rate,
                            download_priority: download_priority.and_then(|p| p.parse().ok()),
                            upload_priority: upload_priority.and_then(|p| p.parse().ok()),
                        },
                    })
                }
                msg if msg.starts_with("Program: ") => {
                    let mut msg = msg.split("Program: ").nth(1)?.split_whitespace();
                    let name = msg.next()?.to_string();
                    let download_rate = parse_part(msg.next());
                    let upload_rate = parse_part(msg.next());
                    let download_minimum_rate = parse_part(msg.next());
                    let upload_minimum_rate = parse_part(msg.next());
                    let download_priority = parse_part(msg.next());
                    let upload_priority = parse_part(msg.next());
                    Some(Program {
                        name,
                        config: LimitConfig {
                            download_rate,
                            download_minimum_rate,
                            upload_rate,
                            upload_minimum_rate,
                            download_priority: download_priority.and_then(|p| p.parse().ok()),
                            upload_priority: upload_priority.and_then(|p| p.parse().ok()),
                        },
                    })
                }
                _ => None,
            }
        };
        parse().ok_or(format!("failed to parse message: {msg}"))
    }
}

#[test]
fn test_parse_message() {
    assert_eq!(
        "Program: firefox 100kbps".to_string().try_into(),
        Ok(Message::Program {
            name: "firefox".into(),
            config: LimitConfig {
                download_rate: Some("100kbps".into()),
                download_minimum_rate: None,
                upload_rate: None,
                upload_minimum_rate: None,
                download_priority: None,
                upload_priority: None,
            }
        })
    );
    assert_eq!(
        "Global: None 10kbps".to_string().try_into(),
        Ok(Message::Global {
            config: LimitConfig {
                download_rate: None,
                download_minimum_rate: None,
                upload_rate: Some("10kbps".into()),
                upload_minimum_rate: None,
                download_priority: None,
                upload_priority: None,
            }
        })
    );
    assert_eq!(
        "Interface: wlan0".to_string().try_into(),
        Ok(Message::Interface("wlan0".into()))
    );
    assert_eq!("Stop".to_string().try_into(), Ok(Message::Stop));
}
