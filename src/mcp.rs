use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::config::json_string;
use crate::dispatch::call_script;
use crate::zenoh_face::Bridge;

pub async fn serve(bridge: Bridge) {
    let listen = bridge.config.mcp_listen.clone();
    let listener = TcpListener::bind(&listen)
        .await
        .unwrap_or_else(|error| panic!("mcp bind {listen}: {error}"));
    println!("mcp http://{listen}/mcp");
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            continue;
        };
        let bridge = bridge.clone();
        tokio::spawn(async move {
            let mut buf = vec![0_u8; 8192];
            let mut stored = Vec::new();
            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                stored.extend_from_slice(&buf[..n]);
                while let Some(request) = take_http(&mut stored) {
                    let (status, body, close) = handle(&bridge, &request);
                    let header = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: {}\r\n\r\n",
                        body.len(),
                        if close { "close" } else { "keep-alive" }
                    );
                    if socket.write_all(header.as_bytes()).await.is_err()
                        || socket.write_all(body.as_bytes()).await.is_err()
                    {
                        return;
                    }
                    if close {
                        return;
                    }
                }
            }
        });
    }
}

fn take_http(buf: &mut Vec<u8>) -> Option<String> {
    let header_end = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    let header = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut length = 0_usize;
    for line in header.split("\r\n") {
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let total = header_end + 4 + length;
    if buf.len() < total {
        return None;
    }
    let body = String::from_utf8_lossy(&buf[header_end + 4..total]).to_string();
    buf.drain(..total);
    Some(body)
}

fn handle(bridge: &Bridge, body: &str) -> (u16, String, bool) {
    if body.trim().is_empty() {
        return (202, String::new(), false);
    }
    let method = json_field(body, "method").unwrap_or_default();
    let id = json_field(body, "id");
    let has_id = body.contains("\"id\"");
    if !has_id {
        return (202, String::new(), false);
    }
    let result = match method.as_str() {
        "initialize" => {
            r#"{"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"zenoh-mcp-device-bridge","version":"0.1.0"},"instructions":"Tools are <device>.<tool> from Zenoh device manifests. Scripted devices run their local command."}"#.to_string()
        }
        "tools/list" => tools_list(bridge),
        "tools/call" => tools_call(bridge, body),
        _ => format!(r#"{{"code":-32601,"message":{}}}"#, json_string("method not found")),
    };
    let payload = if method == "tools/call" && result.contains("\"code\"") {
        format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{result}}}"#, id = id.unwrap_or_else(|| "null".into()))
    } else if result.starts_with('{') && result.contains("\"code\"") && method != "tools/list" && method != "initialize" {
        format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{result}}}"#, id = id.unwrap_or_else(|| "null".into()))
    } else {
        format!(
            r#"{{"jsonrpc":"2.0","id":{},"result":{result}}}"#,
            id.unwrap_or_else(|| "null".into())
        )
    };
    (200, payload, false)
}

fn tools_list(bridge: &Bridge) -> String {
    let mut tools = Vec::new();
    for device in &bridge.config.device {
        for tool in &device.tool {
            tools.push(mcp_tool(&device.id, &tool.name, &tool.description, tool.confirm));
        }
    }
    if let Ok(found) = bridge.discovered.lock() {
        for (id, manifest) in found.iter() {
            if bridge.facade(id).is_some() {
                continue;
            }
            for name in tool_names(manifest) {
                tools.push(mcp_tool(id, &name, "from device manifest", false));
            }
        }
    }
    format!(r#"{{"tools":[{}]}}"#, tools.join(","))
}

fn tools_call(bridge: &Bridge, body: &str) -> String {
    let name = json_field(body, "name").unwrap_or_default();
    let Some((id, tool)) = name.split_once('.') else {
        return format!(r#"{{"code":-32602,"message":{}}}"#, json_string("tool name must be device.tool"));
    };
    let args = arguments_object(body).unwrap_or_default();
    if let Some(device) = bridge.facade(id) {
        let text = call_script(device, tool, &args);
        return format!(r#"{{"content":[{{"type":"text","text":{}}}]}}"#, json_string(&text));
    }
    format!(
        r#"{{"content":[{{"type":"text","text":{}}}],"isError":true}}"#,
        json_string("remote device calls are not forwarded yet")
    )
}

fn mcp_tool(id: &str, name: &str, description: &str, confirm: bool) -> String {
    let required = if confirm {
        r#","required":["confirm"]"#
    } else {
        ""
    };
    let props = if confirm {
        r#""confirm":{"type":"boolean"}"#
    } else {
        ""
    };
    format!(
        r#"{{"name":{},"description":{},"inputSchema":{{"type":"object","properties":{{{props}}}{required}}}}}"#,
        json_string(&format!("{id}.{name}")),
        json_string(description),
    )
}

fn tool_names(manifest: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(manifest) else {
        return Vec::new();
    };
    value
        .get("tools")
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool.get("name")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn json_field(body: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\"");
    let start = body.find(&needle)?;
    let after = &body[start + needle.len()..];
    let after = after.trim_start().trim_start_matches(':').trim_start();
    if let Some(rest) = after.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                break;
            } else {
                out.push(c);
            }
        }
        return Some(out);
    }
    let end = after.find([',', '}', ' ', '\n']).unwrap_or(after.len());
    Some(after[..end].trim().to_string())
}

fn arguments_object(body: &str) -> Option<String> {
    let needle = "\"arguments\"";
    let start = body.find(needle)?;
    let after = body[start + needle.len()..].trim_start().trim_start_matches(':').trim_start();
    if !after.starts_with('{') {
        return None;
    }
    let mut depth = 0_i32;
    for (i, c) in after.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(after[..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}
