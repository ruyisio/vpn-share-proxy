use std::os::unix::io::{FromRawFd, RawFd};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

const ABSTRACT_SOCKET_NAME: &str = "vpn_share_proxy.sock";

fn connect_abstract(name: &str) -> std::io::Result<std::os::unix::net::UnixStream> {
    unsafe {
        let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut addr: libc::sockaddr_un = std::mem::zeroed();
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let bytes = name.as_bytes();
        if bytes.len() + 1 >= addr.sun_path.len() {
            libc::close(fd);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "abstract socket name too long",
            ));
        }
        // sun_path[0] = '\0' => abstract namespace; name follows.
        for (i, &b) in bytes.iter().enumerate() {
            addr.sun_path[1 + i] = b as libc::c_char;
        }
        let len = std::mem::size_of::<libc::sa_family_t>() + 1 + bytes.len();
        if libc::connect(fd, &addr as *const _ as *const libc::sockaddr, len as libc::socklen_t) < 0
        {
            let err = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(err);
        }
        Ok(std::os::unix::net::UnixStream::from_raw_fd(fd as RawFd))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let action = args.get(1).map(|s| s.as_str()).unwrap_or("status");

    let std_stream = match connect_abstract(ABSTRACT_SOCKET_NAME) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Error: cannot connect to @{}: {}. Is vspd running?",
                ABSTRACT_SOCKET_NAME, e
            );
            std::process::exit(1);
        }
    };
    let stream = UnixStream::from_std(std_stream)?;
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    match action {
        "status" => {
            let req = serde_json::json!({
                "id": 1,
                "action": "get_status"
            });
            writer
                .write_all(format!("{}\n", req).as_bytes())
                .await?;

            if let Some(line) = lines.next_line().await? {
                let parsed: serde_json::Value = serde_json::from_str(&line)?;
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            }
        }
        "config" => {
            let req = serde_json::json!({
                "id": 1,
                "action": "get_config"
            });
            writer
                .write_all(format!("{}\n", req).as_bytes())
                .await?;

            if let Some(line) = lines.next_line().await? {
                let parsed: serde_json::Value = serde_json::from_str(&line)?;
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            }
        }
        "devices" => {
            let req = serde_json::json!({
                "id": 1,
                "action": "list_devices"
            });
            writer
                .write_all(format!("{}\n", req).as_bytes())
                .await?;

            if let Some(line) = lines.next_line().await? {
                let parsed: serde_json::Value = serde_json::from_str(&line)?;
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            }
        }
        "reconcile" => {
            let req = serde_json::json!({
                "id": 1,
                "action": "reconcile_now"
            });
            writer
                .write_all(format!("{}\n", req).as_bytes())
                .await?;

            if let Some(line) = lines.next_line().await? {
                let parsed: serde_json::Value = serde_json::from_str(&line)?;
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            }
        }
        "events" | "subscribe" => {
            println!("Subscribing to live event stream from vspd (press Ctrl+C to stop)...");
            let req = serde_json::json!({
                "id": 1,
                "action": "subscribe"
            });
            writer
                .write_all(format!("{}\n", req).as_bytes())
                .await?;

            while let Some(line) = lines.next_line().await? {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                    if parsed.get("event").is_some() {
                        println!("[EVENT] {}", serde_json::to_string_pretty(&parsed)?);
                    } else {
                        println!("[ACK] {}", line);
                    }
                } else {
                    println!("{}", line);
                }
            }
        }
        "mode" => {
            let mode = args.get(2).map(|s| s.as_str()).unwrap_or("global");
            let req = serde_json::json!({
                "id": 1,
                "action": "set_proxy_mode",
                "params": {
                    "mode": mode
                }
            });
            writer.write_all(format!("{}\n", req).as_bytes()).await?;
            if let Some(line) = lines.next_line().await? {
                let parsed: serde_json::Value = serde_json::from_str(&line)?;
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            }
        }
        "set-policy" => {
            let mac = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let policy = args.get(3).map(|s| s.as_str()).unwrap_or("bypass");
            let req = serde_json::json!({
                "id": 1,
                "action": "set_device_policy",
                "params": {
                    "mac": mac,
                    "policy": policy
                }
            });
            writer.write_all(format!("{}\n", req).as_bytes()).await?;
            if let Some(line) = lines.next_line().await? {
                let parsed: serde_json::Value = serde_json::from_str(&line)?;
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            }
        }
        _ => {
            println!("VPN Share Proxy Control Client (vsp-ctl)");
            println!("Usage: vsp-ctl <command> [args]");
            println!("");
            println!("Commands:");
            println!("  status                        Get daemon status and active tun/table");
            println!("  config                        Get current configuration");
            println!("  mode <global|mac_allowlist|mac_blocklist>  Set proxy mode");
            println!("  devices                       List connected hotspot client devices");
            println!("  set-policy <mac> <allow|bypass> Set policy for specific device");
            println!("  reconcile                     Trigger immediate policy reconciliation");
            println!("  events                        Live stream server-push events in real time");
        }
    }

    Ok(())
}
