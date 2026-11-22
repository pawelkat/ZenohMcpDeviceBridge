#!/data/data/com.termux/files/usr/bin/bash
# Termux:Boot. This file sorts before start-zenohd.sh, so it must return.
# The bridge starts in the background once the router is listening.
# Android rejects the static PIE; the binary runs inside Ubuntu proot.
termux-wake-lock
export PROOT_NO_SECCOMP=1
LOG=/data/data/com.termux/files/home/zenoh-mcp-device-bridge.log
BIN=/data/data/com.termux/files/home/bin/zenoh-mcp-device-bridge.proot
CFG=/data/data/com.termux/files/home/.config/zenoh-mcp-device-bridge.toml
if (echo >/dev/tcp/127.0.0.1/8077) >/dev/null 2>&1; then
  exit 0
fi
(
  i=0
  while [ "$i" -lt 30 ]; do
    if (echo >/dev/tcp/127.0.0.1/7447) >/dev/null 2>&1; then
      exec proot-distro login ubuntu -- "$BIN" "$CFG"
    fi
    i=$((i + 1))
    sleep 1
  done
  echo "zenohd :7447 not open; zenoh-mcp-device-bridge not started"
) >>"$LOG" 2>&1 &
disown
exit 0
