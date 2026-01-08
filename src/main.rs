use std::collections::HashMap;
use std::net::{IpAddr, UdpSocket};
use std::process::Command;
use std::thread;
use std::time::Duration;
use serde::Deserialize;

// Configuration
const KEEPALIVE_INTERVAL_SECS: u64 = 25;
const TARGET_PORT: u16 = 41641; 
const PACKET_DATA: &[u8] = b"z"; 

#[derive(Debug, Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "Peer", default)]
    peers: HashMap<String, PeerNode>,
}

#[derive(Debug, Deserialize)]
struct PeerNode {
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,
}

fn main() {
    println!("Starting tailscale-keepalived (IPv4 only)...");
    
    // Bind to local IPv4 port
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to bind UDP socket: {}", e);
            return;
        }
    };
    
    let _ = socket.set_nonblocking(true);

    loop {
        if let Err(e) = run_keepalive_cycle(&socket) {
            eprintln!("Error: {}", e);
        }
        thread::sleep(Duration::from_secs(KEEPALIVE_INTERVAL_SECS));
    }
}

fn run_keepalive_cycle(socket: &UdpSocket) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Get Status
    let output = Command::new("tailscale")
        .arg("status")
        .arg("--json")
        .output()?;

    if !output.status.success() {
        return Err("Tailscale command failed".into());
    }

    let status_json = String::from_utf8(output.stdout)?;
    let status: TailscaleStatus = serde_json::from_str(&status_json)?;

    let mut sent_count = 0;

    // 2. Iterate Peers
    for (_node_key, peer) in status.peers {
        for ip_str in peer.tailscale_ips {
            // Parse string to IP Address
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                // ONLY process IPv4 addresses to avoid "Address family not supported"
                if ip.is_ipv4() {
                    let target = format!("{}:{}", ip, TARGET_PORT);
                    
                    if let Err(e) = socket.send_to(PACKET_DATA, &target) {
                        eprintln!("Failed to send to {}: {}", target, e);
                    } else {
                        sent_count += 1;
                    }
                }
            }
        }
    }

    println!("Keepalive cycle: Sent packets to {} IPv4 peers.", sent_count);
    Ok(())
}