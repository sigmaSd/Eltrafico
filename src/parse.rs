use std::collections::HashMap;

type Rate = Option<String>;
pub fn parse(config: String) -> crate::CatchAll<HashMap<String, (Rate, Rate)>> {
    let mut map = HashMap::new();

    let file_data = std::fs::read_to_string(config)?;
    for line in file_data.lines() {
        if line.starts_with('#') {
            continue;
        }
        let mut line = line.split_whitespace();
        let name = match line.next() {
            Some(n) => n,
            None => continue,
        };
        let line: Vec<&str> = line.collect();

        let down_idx = line.iter().position(|s| s == &"d:").map(|v| (v + 1));
        let down_value = if let Some(down_idx) = down_idx {
            line.get(down_idx).map(ToString::to_string)
        } else {
            None
        };

        let up_idx = line.iter().position(|s| s == &"u:").map(|v| v + 1);
        let up_value = if let Some(up_idx) = up_idx {
            line.get(up_idx).map(ToString::to_string)
        } else {
            None
        };

        map.insert(name.to_string(), (down_value, up_value));
    }
    Ok(map)
}
