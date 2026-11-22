use std::process::Command;

use crate::config::{json_string, Device};

pub fn call_script(device: &Device, tool_name: &str, payload: &str) -> String {
    let Some(tool) = device.tool(tool_name) else {
        return err(&format!("unknown tool {tool_name}"));
    };
    if tool.confirm && !confirm_true(payload) {
        return err(&format!("{tool_name} requires confirm true"));
    }
    let Some(program) = device.command.first() else {
        return err("device command is empty");
    };
    let mut cmd = Command::new(program);
    if device.command.len() > 1 {
        cmd.args(&device.command[1..]);
    }
    cmd.args(&tool.args);
    cmd.env(
        "PATH",
        "/data/data/com.termux/files/usr/bin:/system/bin:/usr/bin:/bin",
    );
    match cmd.output() {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).to_string();
            let err_text = String::from_utf8_lossy(&out.stderr);
            if !err_text.trim().is_empty() {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&err_text);
            }
            let line = summary(&text);
            if out.status.success() {
                ok(&line)
            } else {
                err(&line)
            }
        }
        Err(error) => err(&error.to_string()),
    }
}

fn confirm_true(payload: &str) -> bool {
    let compact: String = payload.split_whitespace().collect();
    compact.contains("\"confirm\":true") || compact.contains("confirm=true")
}

fn summary(out: &str) -> String {
    out.lines()
        .rev()
        .find(|line| line.contains("->"))
        .map(|line| line.split("->").nth(1).unwrap_or(line).trim().to_string())
        .filter(|line| !line.is_empty())
        .or_else(|| {
            out.lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
        })
        .unwrap_or_else(|| "empty reply".into())
}

fn ok(text: &str) -> String {
    format!(r#"{{"ok":true,"result":{{"text":{}}}}}"#, json_string(text))
}

fn err(text: &str) -> String {
    format!(r#"{{"ok":false,"error":{}}}"#, json_string(text))
}
