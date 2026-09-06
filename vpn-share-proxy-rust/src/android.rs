use std::fs;
use std::path::Path;

pub const DEFAULT_MODDIR: &str = "/data/adb/modules/vpn_share_proxy";
pub const RT_TABLES_PATH: &str = "/data/misc/net/rt_tables";
pub const ARP_TABLE_PATH: &str = "/proc/net/arp";

/// Pure Native Android & Linux Environment Helper (0 Command::new calls)
pub struct AndroidEnv;

impl AndroidEnv {
    /// Read Android property directly from /system/build.prop without spawning any process
    pub fn get_property(key: &str) -> Option<String> {
        let prop_files = [
            "/system/build.prop",
            "/vendor/build.prop",
            "/product/build.prop",
            "/system/etc/prop.default",
            "/default.prop",
        ];

        for path in &prop_files {
            if let Ok(content) = fs::read_to_string(path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with('#') || line.is_empty() {
                        continue;
                    }
                    if let Some((k, v)) = line.split_once('=') {
                        if k.trim() == key {
                            return Some(v.trim().to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Android SDK version from build.prop
    pub fn sdk_version() -> u32 {
        Self::get_property("ro.build.version.sdk")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    }

    /// Device model name from build.prop
    pub fn device_model() -> String {
        Self::get_property("ro.product.model").unwrap_or_else(|| "Android Device".to_string())
    }

    /// Detect active Root manager by inspecting directory markers
    pub fn detect_root_manager() -> &'static str {
        if Path::new("/data/adb/ksu").is_dir() {
            "KernelSU"
        } else if Path::new("/data/adb/ap").is_dir() {
            "APatch"
        } else if Path::new("/data/adb/magisk").is_dir() {
            "Magisk"
        } else {
            "Root"
        }
    }

    /// Query /data/misc/net/rt_tables for table index of active VPN tun/wg interface
    pub fn find_vpn_from_rt_tables() -> Option<(String, u32)> {
        let content = fs::read_to_string(RT_TABLES_PATH).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let idx_str = parts[0];
                let name = parts[1];
                if name.starts_with("tun") || name.starts_with("wg") {
                    if let Ok(idx) = idx_str.parse::<u32>() {
                        return Some((name.to_string(), idx));
                    }
                }
            }
        }
        None
    }

    /// Find active VPN tun interface by scanning /sys/class/net
    pub fn find_active_tun_iface() -> Option<String> {
        if let Ok(entries) = fs::read_dir("/sys/class/net") {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.starts_with("tun") || name.starts_with("wg") {
                    let operstate_path = entry.path().join("operstate");
                    if let Ok(state) = fs::read_to_string(operstate_path) {
                        let s = state.trim();
                        if s == "up" || s == "unknown" {
                            return Some(name.to_string());
                        }
                    } else {
                        return Some(name.to_string());
                    }
                }
            }
        }

        Self::find_vpn_from_rt_tables().map(|(name, _)| name)
    }

    /// Look up routing table ID for a given interface from /data/misc/net/rt_tables
    pub fn get_table_for_iface(iface: &str) -> Option<u32> {
        let content = fs::read_to_string(RT_TABLES_PATH).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == iface {
                if let Ok(idx) = parts[0].parse::<u32>() {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Detect active downstream hotspot interfaces by scanning /proc/net/route and /sys/class/net.
    /// Never fall back to wlan0 — on station mode that is the upstream uplink, not the AP.
    pub fn detect_hotspot_interfaces() -> Vec<String> {
        let mut ifaces = Vec::new();

        // 1. Check /proc/net/route for active tethering subnets:
        //    002BA8C0 = 192.168.43.0 (WiFi hotspot)
        //    002AA8C0 = 192.168.42.0 (USB tethering)
        //    002CA8C0 = 192.168.44.0 (Bluetooth tethering)
        if let Ok(content) = fs::read_to_string("/proc/net/route") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let iface = parts[0];
                    let dest = parts[1];
                    if dest == "002BA8C0" || dest == "002AA8C0" || dest == "002CA8C0" {
                        if iface != "wlan0" && !ifaces.contains(&iface.to_string()) {
                            ifaces.push(iface.to_string());
                        }
                    }
                }
            }
        }

        // 2. Common dedicated tethering interface names (must be up/unknown)
        if ifaces.is_empty() {
            let candidates = ["wlan1", "ap0", "softap0", "swlan0", "rndis0", "usb0", "bt-pan"];
            for candidate in candidates {
                let p = format!("/sys/class/net/{}/operstate", candidate);
                if let Ok(state) = fs::read_to_string(&p) {
                    let s = state.trim();
                    if s == "up" || s == "unknown" {
                        ifaces.push(candidate.to_string());
                    }
                }
            }
        }

        ifaces
    }

    /// Heuristic recommend label for display only (may be wrong on some ROMs).
    pub fn iface_hint(name: &str) -> (bool, String) {
        let n = name.to_lowercase();
        if n == "wlan1"
            || n == "ap0"
            || n == "softap0"
            || n == "swlan0"
            || (n.starts_with("wlan") && n != "wlan0")
        {
            return (true, "可能是 Wi-Fi 热点".into());
        }
        if n == "rndis0" || n == "usb0" || n.starts_with("rndis") {
            return (true, "可能是 USB 共享".into());
        }
        if n == "bt-pan" || n.starts_with("bnep") {
            return (true, "可能是蓝牙共享".into());
        }
        // Route-based: if this iface carries classic tether nets
        if let Ok(content) = fs::read_to_string("/proc/net/route") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0] == name {
                    match parts[1] {
                        "002BA8C0" => return (true, "可能是 Wi-Fi 热点".into()),
                        "002AA8C0" => return (true, "可能是 USB 共享".into()),
                        "002CA8C0" => return (true, "可能是蓝牙共享".into()),
                        _ => {}
                    }
                }
            }
        }
        (false, String::new())
    }

    /// Enumerate all network interfaces for the App picker.
    pub fn list_network_interfaces() -> Vec<crate::types::NetIfaceInfo> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir("/sys/class/net") else {
            return out;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let operstate = fs::read_to_string(entry.path().join("operstate"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".into());
            let (recommended, hint) = Self::iface_hint(&name);
            out.push(crate::types::NetIfaceInfo {
                name,
                operstate,
                recommended,
                hint,
                managed: false, // filled by caller from config
            });
        }
        out.sort_by(|a, b| {
            b.recommended
                .cmp(&a.recommended)
                .then_with(|| a.name.cmp(&b.name))
        });
        out
    }

    /// Resolve VPN DNS without spawning processes.
    /// Auto mode falls back to well-known resolvers (VPN app DNS is usually 1.1.1.1 / Cloudflare).
    pub fn get_vpn_dns(v6: bool) -> Option<String> {
        if v6 {
            Some("2606:4700:4700::1111".to_string())
        } else {
            Some("1.1.1.1".to_string())
        }
    }
}
