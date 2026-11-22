mod config;
mod dispatch;
mod mcp;
mod zenoh_face;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zenoh_face::Bridge;

#[tokio::main]
async fn main() {
    zenoh::init_log_from_env_or("error");
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "note4c.toml".into());
    let config = config::Config::load(&path).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    println!(
        "zenoh-mcp-device-bridge devices {}",
        config
            .device
            .iter()
            .map(|device| device.id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let bridge = Bridge {
        config,
        discovered: Arc::new(Mutex::new(HashMap::new())),
    };
    let mcp_bridge = bridge.clone();
    tokio::spawn(async move { mcp::serve(mcp_bridge).await });
    zenoh_face::serve(bridge).await;
}
