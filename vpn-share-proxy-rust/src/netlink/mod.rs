//! AF_NETLINK route/neighbor listener — event-driven only (no heartbeat poll).

use tokio::sync::mpsc;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetlinkEvent {
    /// Link / addr / route changed — may need rule reconcile.
    NetworkChanged,
    /// Neighbor table changed — refresh device sessions only.
    NeighborChanged,
}

pub struct NetlinkMonitor {
    tx: mpsc::Sender<NetlinkEvent>,
}

impl NetlinkMonitor {
    pub fn new(tx: mpsc::Sender<NetlinkEvent>) -> Self {
        Self { tx }
    }

    pub async fn run(self) {
        info!("Starting Linux Netlink kernel event listener");

        let sock_fd = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                libc::NETLINK_ROUTE,
            )
        };

        if sock_fd < 0 {
            warn!("AF_NETLINK unavailable; emergency 30s fallback (no 2s poll)");
            self.run_fallback_timer().await;
            return;
        }

        // RTMGRP_LINK|NOTIFY|NEIGH|IPV4_IFADDR|IPV4_ROUTE|IPV6_IFADDR|IPV6_ROUTE
        let mut sa: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        sa.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        sa.nl_pid = std::process::id() as u32;
        sa.nl_groups = 1 | 2 | 4 | 0x10 | 0x40 | 0x100 | 0x400;

        let res = unsafe {
            libc::bind(
                sock_fd,
                &sa as *const _ as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };

        if res < 0 {
            warn!("Netlink bind failed; emergency 30s fallback");
            unsafe { libc::close(sock_fd) };
            self.run_fallback_timer().await;
            return;
        }

        info!("AF_NETLINK bound (link/addr/route/neigh) — pure event mode");

        let async_fd = match tokio::io::unix::AsyncFd::new(sock_fd) {
            Ok(fd) => fd,
            Err(e) => {
                warn!("AsyncFd failed: {}; emergency 30s fallback", e);
                unsafe { libc::close(sock_fd) };
                self.run_fallback_timer().await;
                return;
            }
        };

        let mut buf = [0u8; 8192];

        loop {
            match async_fd.readable().await {
                Ok(mut guard) => {
                    let n = unsafe {
                        libc::recv(
                            sock_fd,
                            buf.as_mut_ptr() as *mut libc::c_void,
                            buf.len(),
                            0,
                        )
                    };
                    guard.clear_ready();
                    if n <= 0 {
                        continue;
                    }
                    let slice = &buf[..n as usize];
                    let (net, neigh) = classify_nlmsgs(slice);
                    if net {
                        let _ = self.tx.send(NetlinkEvent::NetworkChanged).await;
                    }
                    if neigh {
                        let _ = self.tx.send(NetlinkEvent::NeighborChanged).await;
                    }
                }
                Err(e) => {
                    warn!("Netlink AsyncFd error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
    }

    /// Only if Netlink cannot be opened — slow safety net, not the normal path.
    async fn run_fallback_timer(self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let _ = self.tx.send(NetlinkEvent::NetworkChanged).await;
            let _ = self.tx.send(NetlinkEvent::NeighborChanged).await;
        }
    }
}

const RTM_NEWLINK: u16 = 16;
const RTM_DELLINK: u16 = 17;
const RTM_NEWADDR: u16 = 20;
const RTM_DELADDR: u16 = 21;
const RTM_NEWROUTE: u16 = 24;
const RTM_DELROUTE: u16 = 25;
const RTM_NEWNEIGH: u16 = 28;
const RTM_DELNEIGH: u16 = 29;

fn classify_nlmsgs(buf: &[u8]) -> (bool, bool) {
    let mut net = false;
    let mut neigh = false;
    let mut off = 0usize;
    while off + 16 <= buf.len() {
        let len = u32::from_ne_bytes(buf[off..off + 4].try_into().unwrap()) as usize;
        if len < 16 || off + len > buf.len() {
            break;
        }
        let msg_type = u16::from_ne_bytes(buf[off + 4..off + 6].try_into().unwrap());
        match msg_type {
            RTM_NEWLINK | RTM_DELLINK | RTM_NEWADDR | RTM_DELADDR | RTM_NEWROUTE | RTM_DELROUTE => {
                net = true;
            }
            RTM_NEWNEIGH | RTM_DELNEIGH => {
                neigh = true;
            }
            _ => {}
        }
        // NLMSG_ALIGN(len)
        off += (len + 3) & !3;
    }
    // Unknown/empty batch: treat as network change so we still react
    if !net && !neigh && !buf.is_empty() {
        net = true;
    }
    (net, neigh)
}
