# VPN Share Proxy

Android hotspot → VPN transparent gateway (KernelSU / Magisk).

| Directory | Role |
|-----------|------|
| `vpn-share-proxy-rust/` | Daemon (`vspd`) + CLI (`vsp-ctl`) |
| `vpn-share-proxy-module/` | Magisk / KernelSU module packaging |
| `vpn-share-proxy-app/` | Android control app |

Build each tree with its `Makefile` (`make build-docker`). Artifacts and local config are gitignored.
