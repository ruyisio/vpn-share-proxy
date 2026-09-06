use crate::android::AndroidEnv;
use crate::config::ConfigManager;
use crate::device::DeviceTracker;
use crate::firewall::FirewallManager;
use crate::routing::NetlinkRuleManager;
use crate::types::{DaemonStatus, EventFrame, GatewayConfig, RunningState};
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn};

pub struct Reconciler {
    config_mgr: Arc<ConfigManager>,
    routing_mgr: Arc<NetlinkRuleManager>,
    firewall_mgr: Arc<FirewallManager>,
    device_tracker: Arc<DeviceTracker>,
    event_tx: broadcast::Sender<EventFrame>,
    session: Mutex<ModuleSession>,
    start_time: Instant,
}

/// Currently installed kernel state (module lifecycle).
struct ModuleSession {
    phase: RunningState,
    /// Fingerprint of last successfully entered Desired (0 = nothing installed).
    installed_sig: u64,
    tun: Option<String>,
    table: Option<u32>,
    v4_on: bool,
    v6_on: bool,
}

impl ModuleSession {
    fn idle() -> Self {
        Self {
            phase: RunningState::Standby,
            installed_sig: 0,
            tun: None,
            table: None,
            v4_on: false,
            v6_on: false,
        }
    }

    fn has_rules(&self) -> bool {
        self.installed_sig != 0
    }
}

/// Intent computed from config + environment.
struct Desired {
    phase: RunningState,
    /// Fingerprint when Active; 0 for Standby/WaitingVpn.
    sig: u64,
    tun: Option<String>,
    table: Option<u32>,
    enable_v4: bool,
    enable_v6: bool,
    config: GatewayConfig,
}

impl Reconciler {
    pub fn new(
        config_mgr: Arc<ConfigManager>,
        routing_mgr: Arc<NetlinkRuleManager>,
        firewall_mgr: Arc<FirewallManager>,
        device_tracker: Arc<DeviceTracker>,
        event_tx: broadcast::Sender<EventFrame>,
    ) -> Self {
        Self {
            config_mgr,
            routing_mgr,
            firewall_mgr,
            device_tracker,
            event_tx,
            session: Mutex::new(ModuleSession::idle()),
            start_time: Instant::now(),
        }
    }

    async fn desired_from(&self, config: GatewayConfig) -> Desired {
        if !config.enable_ipv4 && !config.enable_ipv6 {
            return Desired {
                phase: RunningState::Standby,
                sig: 0,
                tun: None,
                table: None,
                enable_v4: false,
                enable_v6: false,
                config,
            };
        }

        let Some(tun) = AndroidEnv::find_active_tun_iface() else {
            return Desired {
                phase: RunningState::WaitingVpn,
                sig: 0,
                tun: None,
                table: None,
                enable_v4: config.enable_ipv4,
                enable_v6: config.enable_ipv6,
                config,
            };
        };

        let mut table_opt = AndroidEnv::get_table_for_iface(&tun);
        if table_opt.is_none() {
            for _ in 0..3 {
                tokio::time::sleep(Duration::from_millis(150)).await;
                table_opt = AndroidEnv::get_table_for_iface(&tun);
                if table_opt.is_some() {
                    break;
                }
            }
        }
        let table = table_opt.unwrap_or(1038);
        let sig = config.signature(Some(&tun), Some(table));

        Desired {
            phase: RunningState::Active,
            sig,
            tun: Some(tun),
            table: Some(table),
            enable_v4: config.enable_ipv4,
            enable_v6: config.enable_ipv6,
            config,
        }
    }

    fn leave(&self, session: &mut ModuleSession) {
        if !session.has_rules() && session.phase != RunningState::Active {
            session.phase = RunningState::Standby;
            session.v4_on = false;
            session.v6_on = false;
            session.tun = None;
            session.table = None;
            return;
        }
        info!("ModuleSession leave (flush rules)");
        self.routing_mgr.flush_managed_rules(true, true);
        self.firewall_mgr.cleanup();
        *session = ModuleSession::idle();
    }

    fn enter(&self, session: &mut ModuleSession, desired: &Desired) -> Result<()> {
        match desired.phase {
            RunningState::Standby => {
                session.phase = RunningState::Standby;
                let _ = self.event_tx.send(EventFrame::new(
                    "state_changed",
                    serde_json::json!({ "state": "standby" }),
                ));
            }
            RunningState::WaitingVpn => {
                session.phase = RunningState::WaitingVpn;
                let _ = self.event_tx.send(EventFrame::new(
                    "vpn_changed",
                    serde_json::json!({ "status": "down" }),
                ));
            }
            RunningState::Active => {
                let tun = desired.tun.as_deref().unwrap();
                let table = desired.table.unwrap();
                info!(
                    "ModuleSession enter Active tun={} table={} v4={} v6={}",
                    tun, table, desired.enable_v4, desired.enable_v6
                );

                self.firewall_mgr
                    .setup_kernel_forwarding(desired.enable_v4, desired.enable_v6);

                let managed = desired.config.managed_ifaces_sorted();
                let mode = &desired.config.proxy_mode;
                let mut v4_ok = false;
                if desired.enable_v4 {
                    match self.routing_mgr.apply_ipv4_rules(tun, table, mode, &managed) {
                        Ok(()) => v4_ok = true,
                        Err(e) => warn!("IPv4 Netlink rules failed: {}", e),
                    }
                }
                let mut v6_ok = false;
                if desired.enable_v6 {
                    match self.routing_mgr.apply_ipv6_rules(tun, table, mode, &managed) {
                        Ok(()) => v6_ok = true,
                        Err(e) => warn!("IPv6 Netlink rules failed: {}", e),
                    }
                }

                if desired.enable_v4 {
                    self.firewall_mgr.apply_proxy_rules(&desired.config, tun);
                }
                // Always touch v6 chains while Active: accept when on, REJECT leak when off.
                self.firewall_mgr.apply_ipv6_proxy_rules(
                    &desired.config,
                    tun,
                    desired.enable_v6,
                );

                session.phase = RunningState::Active;
                session.installed_sig = desired.sig;
                session.tun = Some(tun.to_string());
                session.table = Some(table);
                session.v4_on = v4_ok;
                session.v6_on = v6_ok;

                let _ = self.event_tx.send(EventFrame::new(
                    "vpn_changed",
                    serde_json::json!({
                        "status": "up",
                        "iface": tun,
                        "table_id": table
                    }),
                ));
                let _ = self.event_tx.send(EventFrame::new(
                    "reconcile_finished",
                    serde_json::json!({
                        "success": true,
                        "active_tun": tun,
                        "table": table,
                        "ipv4": v4_ok,
                        "ipv6": v6_ok
                    }),
                ));
            }
            RunningState::Error => {
                session.phase = RunningState::Error;
            }
        }
        Ok(())
    }

    /// Single path: compute Desired, leave if needed, enter if needed.
    pub async fn reconcile(&self) -> Result<()> {
        let mut session = self.session.lock().await;
        let config = self.config_mgr.load();
        let desired = self.desired_from(config).await;

        // Same Active install — only re-pin FORWARD hooks (tetherctrl race).
        if desired.phase == RunningState::Active
            && desired.sig == session.installed_sig
            && session.phase == RunningState::Active
        {
            self.firewall_mgr.ensure_hooks_pinned();
            return Ok(());
        }

        // Already in non-Active phase with no rules and same phase — noop.
        if desired.phase != RunningState::Active
            && session.phase == desired.phase
            && !session.has_rules()
        {
            return Ok(());
        }

        let need_leave = session.has_rules()
            || (session.phase == RunningState::Active && desired.phase != RunningState::Active);
        if need_leave {
            self.leave(&mut session);
        }

        self.enter(&mut session, &desired)?;
        Ok(())
    }

    pub async fn cleanup_all(&self) {
        let mut session = self.session.lock().await;
        self.leave(&mut session);
    }

    pub async fn get_status(&self) -> DaemonStatus {
        let session = self.session.lock().await;
        DaemonStatus {
            state: session.phase,
            tun_interface: session.tun.clone(),
            table_id: session.table,
            hotspot_interfaces: AndroidEnv::detect_hotspot_interfaces(),
            ipv4_active: session.v4_on,
            ipv6_active: session.v6_on,
            client_count: self.device_tracker.count(),
            uptime_secs: self.start_time.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}
