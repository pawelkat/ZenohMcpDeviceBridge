use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zenoh::query::Query;

use crate::config::Config;
use crate::dispatch::call_script;

#[derive(Clone)]
pub struct Bridge {
    pub config: Config,
    pub discovered: Arc<Mutex<HashMap<String, String>>>,
}

impl Bridge {
    pub fn facade(&self, id: &str) -> Option<&crate::config::Device> {
        self.config.device.iter().find(|device| device.id == id)
    }

    pub fn answer(&self, key: &str, payload: &str) -> Option<String> {
        let rest = key.strip_prefix("devices/")?;
        if let Some(id) = rest.strip_suffix("/manifest") {
            return self.facade(id).map(|device| device.manifest());
        }
        let (id, tool) = rest.split_once("/call/")?;
        if tool.is_empty() || tool.contains('/') {
            return None;
        }
        self.facade(id)
            .map(|device| call_script(device, tool, payload))
    }
}

pub async fn serve(bridge: Bridge) {
    let mut config = zenoh::Config::default();
    config.insert_json5("mode", r#""client""#).expect("mode");
    config
        .insert_json5(
            "connect/endpoints",
            &format!(r#"["{}"]"#, bridge.config.connect),
        )
        .expect("endpoint");
    config
        .insert_json5("scouting/multicast/enabled", "false")
        .expect("scouting");
    let session = zenoh::open(config).await.expect("zenoh open");
    println!("zenoh connected {}", bridge.config.connect);

    for device in &bridge.config.device {
        let key = format!("devices/{}/**", device.id);
        println!("facade {key}");
        let queryable = session
            .declare_queryable(&key)
            .complete(true)
            .await
            .expect("queryable");
        let bridge_queries = bridge.clone();
        tokio::spawn(async move {
            while let Ok(query) = queryable.recv_async().await {
                reply(bridge_queries.clone(), query).await;
            }
            eprintln!("zenoh queryable closed");
        });
    }

    let bridge_liveliness = bridge.clone();
    let session_liveliness = session.clone();
    tokio::spawn(async move {
        let subscriber = session_liveliness
            .liveliness()
            .declare_subscriber("devices/*")
            .history(true)
            .await
            .expect("liveliness");
        println!("watching liveliness devices/*");
        while let Ok(sample) = subscriber.recv_async().await {
            let key = sample.key_expr().to_string();
            let Some(id) = key.strip_prefix("devices/") else {
                continue;
            };
            if id.contains('/') {
                continue;
            }
            if bridge_liveliness.facade(id).is_some() {
                println!("liveliness {id} (facade)");
                continue;
            }
            match sample.kind() {
                zenoh::sample::SampleKind::Put => {
                    println!("liveliness {id}");
                    if let Some(manifest) =
                        fetch_manifest(&session_liveliness, id).await
                    {
                        bridge_liveliness
                            .discovered
                            .lock()
                            .expect("discovered")
                            .insert(id.to_string(), manifest);
                        println!("manifest cached {id}");
                    }
                }
                zenoh::sample::SampleKind::Delete => {
                    bridge_liveliness
                        .discovered
                        .lock()
                        .expect("discovered")
                        .remove(id);
                    println!("liveliness gone {id}");
                }
            }
        }
    });

    std::future::pending::<()>().await;
}

async fn reply(bridge: Bridge, query: Query) {
    let key = query.key_expr().to_string();
    let payload = query
        .payload()
        .map(|bytes| bytes.try_to_string().unwrap_or_default().into_owned())
        .unwrap_or_default();
    let owned = key.clone();
    let body = tokio::task::spawn_blocking(move || bridge.answer(&owned, &payload))
        .await
        .ok()
        .flatten();
    let Some(body) = body else {
        return;
    };
    if let Err(error) = query.reply(key, body).await {
        eprintln!("reply failed: {error}");
    }
}

async fn fetch_manifest(session: &zenoh::Session, id: &str) -> Option<String> {
    let key = format!("devices/{id}/manifest");
    let replies = tokio::time::timeout(Duration::from_secs(3), session.get(&key))
        .await
        .ok()?
        .ok()?;
    let mut found = None;
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while let Ok(reply) = replies.recv_async().await {
            if let Ok(sample) = reply.result() {
                if let Ok(text) = sample.payload().try_to_string() {
                    found = Some(text.into_owned());
                    break;
                }
            }
        }
    })
    .await;
    found
}
