use crate::android::{AndroidEnv, ARP_TABLE_PATH};
use crate::types::{ConnectedDevice, GatewayConfig, ProxyMode};
use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const DEVICE_TTL_SECS: u64 = 600;

pub struct DeviceTracker {
    devices: Arc<RwLock<HashMap<String, ConnectedDevice>>>,
}

impl DeviceTracker {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Refresh from ARP: enter / touch / leave device sessions.
    /// Allow/block: only `managed_ifaces`. Global: no per-device tracking.
    pub fn refresh(&self, config: &GatewayConfig) -> (Vec<ConnectedDevice>, Vec<String>) {
        let tethered = AndroidEnv::detect_hotspot_interfaces();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let discovered = Self::scan_arp(&tethered, config);
        let mut newly_connected = Vec::new();
        let mut disconnected = Vec::new();
        let mut lock = self.devices.write().unwrap();

        for (mac, (ip, iface)) in discovered {
            let policy = Self::policy_for_mac(&mac, config).to_string();
            if let Some(existing) = lock.get_mut(&mac) {
                Self::touch(existing, ip, iface, policy, now);
            } else {
                let dev = Self::enter(mac.clone(), ip, iface, policy, now);
                newly_connected.push(dev.clone());
                lock.insert(mac, dev);
            }
        }

        // Drop out-of-scope devices (global / unmanaged iface) or TTL expired.
        lock.retain(|mac, dev| {
            let in_scope = match config.proxy_mode {
                ProxyMode::Global => false,
                ProxyMode::MacAllowlist | ProxyMode::MacBlocklist => {
                    config.managed_ifaces.contains(&dev.iface)
                }
            };
            if !in_scope || now.saturating_sub(dev.last_seen) > DEVICE_TTL_SECS {
                disconnected.push(mac.clone());
                false
            } else {
                true
            }
        });

        (newly_connected, disconnected)
    }

    fn enter(mac: String, ip: IpAddr, iface: String, policy: String, now: u64) -> ConnectedDevice {
        ConnectedDevice {
            ip,
            mac,
            iface,
            hostname: None,
            policy,
            first_seen: now,
            last_seen: now,
        }
    }

    fn touch(dev: &mut ConnectedDevice, ip: IpAddr, iface: String, policy: String, now: u64) {
        dev.last_seen = now;
        dev.ip = ip;
        dev.iface = iface;
        dev.policy = policy;
    }

    fn iface_in_scope(iface: &str, _ip_str: &str, _tethered: &[String], config: &GatewayConfig) -> bool {
        // Device management only in allow/block modes, and only on user-selected NICs.
        match config.proxy_mode {
            ProxyMode::Global => false,
            ProxyMode::MacAllowlist | ProxyMode::MacBlocklist => {
                !config.managed_ifaces.is_empty() && config.managed_ifaces.contains(iface)
            }
        }
    }

    fn scan_arp(
        tethered: &[String],
        config: &GatewayConfig,
    ) -> HashMap<String, (IpAddr, String)> {
        let mut discovered = HashMap::new();
        let Ok(content) = fs::read_to_string(ARP_TABLE_PATH) else {
            return discovered;
        };

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("IP") || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 {
                continue;
            }
            let ip_str = parts[0];
            let flags = parts[2];
            let mac = parts[3].to_uppercase();
            let iface = parts[5];

            if mac == "00:00:00:00:00:00" || flags == "0x0" || flags == "0x00" {
                continue;
            }

            if !Self::iface_in_scope(iface, ip_str, tethered, config) {
                continue;
            }
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                discovered.insert(mac, (ip, iface.to_string()));
            }
        }
        discovered
    }

    pub fn list_with_policy(&self, config: &GatewayConfig) -> Vec<ConnectedDevice> {
        if matches!(config.proxy_mode, ProxyMode::Global) {
            return Vec::new();
        }
        let lock = self.devices.read().unwrap();
        lock.values()
            .filter(|dev| config.managed_ifaces.contains(&dev.iface))
            .map(|dev| {
                let mut d = dev.clone();
                d.policy = Self::policy_for_mac(&d.mac, config).to_string();
                d
            })
            .collect()
    }

    pub fn policy_for_mac(mac: &str, config: &GatewayConfig) -> &'static str {
        match config.proxy_mode {
            ProxyMode::MacAllowlist => {
                if config.mac_allow_list.contains(mac) {
                    "allowed"
                } else {
                    "bypass"
                }
            }
            ProxyMode::MacBlocklist => {
                if config.mac_block_list.contains(mac) {
                    "bypass"
                } else {
                    "allowed"
                }
            }
            ProxyMode::Global => "allowed",
        }
    }

    pub fn count(&self) -> usize {
        self.devices.read().unwrap().len()
    }
}
