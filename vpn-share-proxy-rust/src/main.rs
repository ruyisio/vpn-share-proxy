mod android;
mod config;
mod device;
mod engine;
mod firewall;
mod ipc;
mod netlink;
mod routing;
mod types;

use android::AndroidEnv;
use config::ConfigManager;
use device::DeviceTracker;
use engine::Reconciler;
use firewall::FirewallManager;
use ipc::IpcServer;
use netlink::{NetlinkEvent, NetlinkMonitor};
use routing::NetlinkRuleManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use types::EventFrame;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().collect();

    // 2. Banner
    let sdk = AndroidEnv::sdk_version();
    let model = AndroidEnv::device_model();
    let root = AndroidEnv::detect_root_manager();

    info!("==================================================");
    info!(
        "VPN Share Proxy Daemon (vspd) v{} starting",
        env!("CARGO_PKG_VERSION")
    );
    info!("Platform: Android (SDK {}), Model: {}", sdk, model);
    info!("Root Environment: {}", root);
    info!("Mode: Netlink FIB rules + kernel-probed netfilter");
    info!("==================================================");

    // 3. Initialize core components
    let config_mgr = Arc::new(ConfigManager::new());
    let routing_mgr = Arc::new(NetlinkRuleManager::new()?);
    let firewall_mgr = Arc::new(FirewallManager::new()?);
    let device_tracker = Arc::new(DeviceTracker::new());
    let (event_tx, _) = broadcast::channel::<EventFrame>(128);

    // DNS stays with Android dnsmasq + VPN table routing (never bind :53 here).

    let reconciler = Arc::new(Reconciler::new(
        Arc::clone(&config_mgr),
        Arc::clone(&routing_mgr),
        Arc::clone(&firewall_mgr),
        Arc::clone(&device_tracker),
        event_tx.clone(),
    ));

    // Handle CLI subcommands
    if args.len() > 1 {
        match args[1].as_str() {
            "--reconcile-once" | "-r" => {
                info!("Running single-shot reconciliation...");
                reconciler.reconcile().await?;
                return Ok(());
            }
            "--cleanup" | "-c" => {
                info!("Cleaning up all rules and stubs...");
                reconciler.cleanup_all().await;
                return Ok(());
            }
            "--version" | "-v" => {
                println!("vspd v{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("VPN Share Proxy Daemon (vspd) - Pure Native");
                println!("Usage: vspd [OPTIONS]");
                println!("  --reconcile-once, -r    Execute one-shot Netlink reconciliation and exit");
                println!("  --cleanup, -c           Flush all Netlink rules and exit");
                println!("  --version, -v           Show version");
                println!("  (no flags)              Run in background daemon mode");
                return Ok(());
            }
            _ => {}
        }
    }

    // 5. Initial boot reconciliation
    info!("Performing initial boot reconciliation via Netlink...");
    if let Err(e) = reconciler.reconcile().await {
        warn!("Initial reconciliation encountered warning: {}", e);
    }

    // 6. Start IPC Server
    let ipc_server = Arc::new(IpcServer::new(
        Arc::clone(&reconciler),
        Arc::clone(&config_mgr),
        Arc::clone(&device_tracker),
        event_tx.clone(),
    ));

    tokio::spawn(async move {
        if let Err(e) = ipc_server.run().await {
            error!("IPC Server stopped: {}", e);
        }
    });

    // 7. Start Netlink Monitor with Debounced Reconciler Queue
    let (nl_tx, mut nl_rx) = mpsc::channel::<NetlinkEvent>(64);
    let netlink_monitor = NetlinkMonitor::new(nl_tx);

    tokio::spawn(async move {
        netlink_monitor.run().await;
    });

    // 8. Setup graceful shutdown signal handling
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    // 9. Event loop — Netlink only (200ms debounce). No periodic ARP/reconcile scan.
    let mut net_debounce: Option<tokio::time::Instant> = None;
    let mut neigh_debounce: Option<tokio::time::Instant> = None;

    // One-shot device scan at boot
    {
        let cfg = config_mgr.load();
        let _ = device_tracker.refresh(&cfg);
    }

    info!("Event orchestrator ready (Netlink events only; no interval poll)");

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                info!("SIGTERM received, shutting down gracefully...");
                break;
            }
            _ = sigint.recv() => {
                info!("SIGINT received, shutting down gracefully...");
                break;
            }
            Some(nl_event) = nl_rx.recv() => {
                match nl_event {
                    NetlinkEvent::NetworkChanged => {
                        net_debounce = Some(tokio::time::Instant::now() + Duration::from_millis(200));
                    }
                    NetlinkEvent::NeighborChanged => {
                        neigh_debounce = Some(tokio::time::Instant::now() + Duration::from_millis(200));
                    }
                }
            }
            _ = async {
                match net_debounce {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => futures::future::pending().await,
                }
            } => {
                net_debounce = None;
                // Link/route change: reconcile rules, then refresh devices (new AP iface etc.)
                if let Err(e) = reconciler.reconcile().await {
                    warn!("Reconciliation error: {}", e);
                }
                publish_device_delta(&config_mgr, &device_tracker, &event_tx);
            }
            _ = async {
                match neigh_debounce {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => futures::future::pending().await,
                }
            } => {
                neigh_debounce = None;
                publish_device_delta(&config_mgr, &device_tracker, &event_tx);
            }
        }
    }

    // Graceful teardown
    info!("Cleaning up network state before exit...");
    reconciler.cleanup_all().await;
    info!("VPN Gateway daemon stopped");
    Ok(())
}

fn publish_device_delta(
    config_mgr: &Arc<ConfigManager>,
    device_tracker: &Arc<DeviceTracker>,
    event_tx: &broadcast::Sender<EventFrame>,
) {
    let cfg = config_mgr.load();
    let (connected, disconnected) = device_tracker.refresh(&cfg);
    for dev in connected {
        info!(
            "Hotspot client connected: {} ({}) on {}",
            dev.ip, dev.mac, dev.iface
        );
        let _ = event_tx.send(EventFrame::new(
            "device_connected",
            serde_json::json!({
                "ip": dev.ip.to_string(),
                "mac": dev.mac,
                "iface": dev.iface,
            }),
        ));
    }
    for mac in disconnected {
        info!("Hotspot client disconnected: {}", mac);
        let _ = event_tx.send(EventFrame::new(
            "device_disconnected",
            serde_json::json!({ "mac": mac }),
        ));
    }
}
