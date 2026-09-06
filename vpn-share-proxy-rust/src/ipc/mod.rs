use crate::android::AndroidEnv;
pub const ABSTRACT_SOCKET_NAME: &str = "vpn_share_proxy.sock";
use crate::config::ConfigManager;
use crate::device::DeviceTracker;
use crate::engine::Reconciler;
use crate::types::{EventFrame, GatewayConfig, RpcRequest, RpcResponse};
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

pub struct IpcServer {
    reconciler: Arc<Reconciler>,
    config_mgr: Arc<ConfigManager>,
    device_tracker: Arc<DeviceTracker>,
    event_tx: broadcast::Sender<EventFrame>,
}

impl IpcServer {
    pub fn new(
        reconciler: Arc<Reconciler>,
        config_mgr: Arc<ConfigManager>,
        device_tracker: Arc<DeviceTracker>,
        event_tx: broadcast::Sender<EventFrame>,
    ) -> Self {
        Self {
            reconciler,
            config_mgr,
            device_tracker,
            event_tx,
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        // Single IPC endpoint: Linux abstract namespace (@vpn_share_proxy.sock)
        let abs_listener = Self::bind_abstract_socket(ABSTRACT_SOCKET_NAME).map_err(|e| {
            anyhow::anyhow!(
                "Failed to bind abstract socket @{}: {}",
                ABSTRACT_SOCKET_NAME,
                e
            )
        })?;
        info!("IPC Abstract Socket listening on @{}", ABSTRACT_SOCKET_NAME);
        Self::accept_loop(abs_listener, self).await;
        Ok(())
    }

    fn bind_abstract_socket(name: &str) -> Result<UnixListener> {
        use std::os::unix::io::FromRawFd;
        unsafe {
            let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC, 0);
            if fd < 0 {
                return Err(anyhow::anyhow!("Failed to create AF_UNIX socket: {}", std::io::Error::last_os_error()));
            }
            let mut addr: libc::sockaddr_un = std::mem::zeroed();
            addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
            let bytes = name.as_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                addr.sun_path[1 + i] = b as libc::c_char;
            }
            let len = std::mem::size_of::<libc::sa_family_t>() + 1 + bytes.len();
            if libc::bind(fd, &addr as *const _ as *const libc::sockaddr, len as libc::socklen_t) < 0 {
                let err = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(anyhow::anyhow!("Failed to bind abstract socket: {}", err));
            }
            if libc::listen(fd, 128) < 0 {
                let err = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(anyhow::anyhow!("Failed to listen on abstract socket: {}", err));
            }
            let std_l = std::os::unix::net::UnixListener::from_raw_fd(fd);
            Ok(UnixListener::from_std(std_l)?)
        }
    }

    async fn accept_loop(listener: UnixListener, server: Arc<Self>) {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let s = Arc::clone(&server);
                    tokio::spawn(async move {
                        if let Err(e) = s.handle_client(stream).await {
                            debug!("Client connection closed: {}", e);
                        }
                    });
                }
                Err(e) => {
                    warn!("Error accepting socket connection: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    async fn handle_client(&self, stream: UnixStream) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        let mut event_rx = self.event_tx.subscribe();
        let mut is_subscribed = false;

        loop {
            tokio::select! {
                line = lines.next_line() => {
                    match line {
                        Ok(Some(msg)) => {
                            let trimmed = msg.trim();
                            if trimmed.is_empty() {
                                continue;
                            }

                            let req: RpcRequest = match serde_json::from_str(trimmed) {
                                Ok(r) => r,
                                Err(e) => {
                                    let err_resp = RpcResponse::error(
                                        serde_json::Value::Null,
                                        -32700,
                                        format!("Invalid JSON: {}", e),
                                    );
                                    let mut json = serde_json::to_string(&err_resp)?;
                                    json.push('\n');
                                    writer.write_all(json.as_bytes()).await?;
                                    continue;
                                }
                            };

                            let resp = self.dispatch_rpc(&req, &mut is_subscribed).await;
                            let mut json = serde_json::to_string(&resp)?;
                            json.push('\n');
                            writer.write_all(json.as_bytes()).await?;
                        }
                        Ok(None) => break, // Connection closed by client
                        Err(e) => return Err(e.into()),
                    }
                }
                // Push server-sent events to subscribed clients
                event = event_rx.recv(), if is_subscribed => {
                    if let Ok(ev) = event {
                        let mut json = serde_json::to_string(&ev)?;
                        json.push('\n');
                        if let Err(e) = writer.write_all(json.as_bytes()).await {
                            debug!("Failed writing event to client: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn dispatch_rpc(&self, req: &RpcRequest, is_subscribed: &mut bool) -> RpcResponse {
        let id = req.id.clone();
        match req.action.as_str() {
            "get_status" => {
                let status = self.reconciler.get_status().await;
                match serde_json::to_value(status) {
                    Ok(val) => RpcResponse::success(id, val),
                    Err(e) => RpcResponse::error(id, -1, e.to_string()),
                }
            }
            "get_config" => {
                let config = self.config_mgr.load();
                match serde_json::to_value(config) {
                    Ok(val) => RpcResponse::success(id, val),
                    Err(e) => RpcResponse::error(id, -1, e.to_string()),
                }
            }
            "list_interfaces" => {
                let config = self.config_mgr.load();
                let mut ifaces = AndroidEnv::list_network_interfaces();
                for iface in &mut ifaces {
                    iface.managed = config.managed_ifaces.contains(&iface.name);
                }
                match serde_json::to_value(ifaces) {
                    Ok(val) => RpcResponse::success(id, val),
                    Err(e) => RpcResponse::error(id, -1, e.to_string()),
                }
            }
            "set_config" => {
                match serde_json::from_value::<GatewayConfig>(req.params.clone()) {
                    Ok(new_cfg) => {
                        if let Err(e) = self.config_mgr.save(&new_cfg) {
                            return RpcResponse::error(id, -1, format!("Failed saving config: {}", e));
                        }
                        let _ = self.reconciler.reconcile().await;
                        RpcResponse::ok(id, "Configuration saved and applied")
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        let hint = if msg.contains("custom_dns") || msg.contains("Ipv4") || msg.contains("Ipv6")
                        {
                            format!("Invalid config (check DNS IP format): {}", msg)
                        } else {
                            format!("Invalid config params: {}", msg)
                        };
                        RpcResponse::error(id, -32602, hint)
                    }
                }
            }
            "set_proxy_mode" => {
                let mode_str = req.params.get("mode").and_then(|v| v.as_str());
                match mode_str {
                    Some("global") | Some("mac_allowlist") | Some("mac_blocklist") => {
                        let mut cfg = self.config_mgr.load();
                        cfg.proxy_mode = match mode_str.unwrap() {
                            "mac_allowlist" => crate::types::ProxyMode::MacAllowlist,
                            "mac_blocklist" => crate::types::ProxyMode::MacBlocklist,
                            _ => crate::types::ProxyMode::Global,
                        };
                        if let Err(e) = self.config_mgr.save(&cfg) {
                            return RpcResponse::error(id, -1, format!("Failed saving config: {}", e));
                        }
                        let _ = self.reconciler.reconcile().await;
                        RpcResponse::ok(id, format!("Proxy mode set to {}", mode_str.unwrap()))
                    }
                    _ => RpcResponse::error(id, -32602, "Invalid mode: use global/mac_allowlist/mac_blocklist"),
                }
            }
            "list_devices" => {
                let config = self.config_mgr.load();
                // On-demand ARP refresh when App pulls (replaces periodic scan).
                let _ = self.device_tracker.refresh(&config);
                let devices = self.device_tracker.list_with_policy(&config);
                match serde_json::to_value(devices) {
                    Ok(val) => RpcResponse::success(id, val),
                    Err(e) => RpcResponse::error(id, -1, e.to_string()),
                }
            }
            "set_device_policy" => {
                let mac = req.params.get("mac").and_then(|v| v.as_str());
                let policy = req.params.get("policy").and_then(|v| v.as_str());

                match (mac, policy) {
                    (Some(m), Some(p)) => {
                        let mac_upper = m.to_uppercase();
                        let mut cfg = self.config_mgr.load();
                        match p {
                            "allow" => {
                                cfg.mac_block_list.remove(&mac_upper);
                                match cfg.proxy_mode {
                                    crate::types::ProxyMode::MacAllowlist => {
                                        cfg.mac_allow_list.insert(mac_upper.clone());
                                    }
                                    crate::types::ProxyMode::Global => {
                                        // Global already routes everyone; clear stale list entries only.
                                    }
                                    crate::types::ProxyMode::MacBlocklist => {
                                        // Not in block list => routed via VPN
                                    }
                                }
                            }
                            "block" | "bypass" => {
                                cfg.mac_allow_list.remove(&mac_upper);
                                match cfg.proxy_mode {
                                    crate::types::ProxyMode::MacAllowlist => {
                                        // Not in allow list => direct (fwmark bypass)
                                    }
                                    crate::types::ProxyMode::Global => {
                                        // Per-device bypass requires blocklist mangle rules
                                        cfg.proxy_mode = crate::types::ProxyMode::MacBlocklist;
                                        cfg.mac_block_list.insert(mac_upper.clone());
                                    }
                                    crate::types::ProxyMode::MacBlocklist => {
                                        cfg.mac_block_list.insert(mac_upper.clone());
                                    }
                                }
                            }
                            "default" => {
                                cfg.mac_allow_list.remove(&mac_upper);
                                cfg.mac_block_list.remove(&mac_upper);
                            }
                            _ => {
                                return RpcResponse::error(
                                    id,
                                    -32602,
                                    "Invalid policy: use allow/block/bypass/default",
                                )
                            }
                        }

                        if let Err(e) = self.config_mgr.save(&cfg) {
                            return RpcResponse::error(id, -1, format!("Failed to save config: {}", e));
                        }
                        if let Err(e) = self.reconciler.reconcile().await {
                            return RpcResponse::error(id, -1, format!("Failed to apply policy: {}", e));
                        }
                        let _ = self.event_tx.send(EventFrame::new(
                            "device_policy_changed",
                            serde_json::json!({ "mac": mac_upper, "policy": p }),
                        ));
                        RpcResponse::ok(id, "Device policy updated")
                    }
                    _ => RpcResponse::error(id, -32602, "Missing 'mac' or 'policy' parameter"),
                }
            }
            "reconcile_now" => {
                match self.reconciler.reconcile().await {
                    Ok(_) => RpcResponse::ok(id, "Reconciled successfully"),
                    Err(e) => RpcResponse::error(id, -1, format!("Reconciliation failed: {}", e)),
                }
            }
            "subscribe" => {
                *is_subscribed = true;
                RpcResponse::ok(id, "Subscribed to live events")
            }
            _ => RpcResponse::error(id, -32601, format!("Method '{}' not found", req.action)),
        }
    }
}
