# Zenoh MCP Device Bridge

Zenoh is the wiring between devices. MCP is the front door for agents.

This is the starting point. It runs on note4c beside `zenohd` and answers `devices/<id>/manifest` and `devices/<id>/call/<tool>` for the devices in the config, by running a local command.

The first device is TowerX power (`towerx-power`): `status`, `power_on`, and `power_off`. The ESP32 keeps its own liveliness token `devices/towerx-power`. This process does not declare that token. Calls for any other device are not forwarded yet.

## Run

```bash
zenoh-mcp-device-bridge note4c.toml
```

`connect` is the router (`tcp/127.0.0.1:7447` on note4c). MCP listens on `mcp_listen` (`0.0.0.0:8077`). Tool names are `<id>.<tool>`.

`power_off` runs only when the Zenoh or MCP payload contains `"confirm":true`.

## note4c

The phone binary is a static PIE for `armv7-unknown-linux-musleabihf`. Android cannot load it, so Termux:Boot starts it inside Ubuntu proot with `PROOT_NO_SECCOMP=1`. The boot script is `termux/start-zenoh-mcp-device-bridge.sh`.

Cross-compile needs Zig 0.14.1 at `~/.local/opt/zig` and the wrappers in this directory:

```bash
export CC_armv7_unknown_linux_musleabihf="$PWD/armv7-musl-cc"
export AR_armv7_unknown_linux_musleabihf="$PWD/armv7-musl-ar"
cargo build --release --target armv7-unknown-linux-musleabihf
```

`.cargo/config.toml` points the linker at the wrapper on this machine.
