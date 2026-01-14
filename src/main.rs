use clap::Parser;
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

/// Tailscale Keepalive Tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Only send packets to devices having at least one of these tags (comma-separated).
    /// Example: --with-tags tag:server,tag:prod
    #[arg(long, value_delimiter = ',', num_args = 0..)]
    with_tags: Option<Vec<String>>,

    /// Do NOT send packets to devices having any of these tags (comma-separated).
    /// Example: --without-tags tag:test
    #[arg(long, value_delimiter = ',', num_args = 0..)]
    without_tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "Peer", default)]
    peers: HashMap<String, PeerNode>,
}

#[derive(Debug, Deserialize)]
struct PeerNode {
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,

    // Field to check if device is online
    #[serde(rename = "Online")]
    online: Option<bool>,

    // Field to check for tags
    #[serde(rename = "Tags")]
    tags: Option<Vec<String>>,
}

fn main() {
    // Parse command line arguments
    let args = Args::parse();

    println!("Starting tailscale-keepalived (IPv4 only)...");
    
    if let Some(tags) = &args.with_tags {
        println!("  Filter: Including only tags: {:?}", tags);
    }
    if let Some(tags) = &args.without_tags {
        println!("  Filter: Excluding tags: {:?}", tags);
    }
    
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
        // Pass the arguments to the cycle function
        if let Err(e) = run_keepalive_cycle(&socket, &args) {
            eprintln!("Error: {}", e);
        }
        thread::sleep(Duration::from_secs(KEEPALIVE_INTERVAL_SECS));
    }
}

fn run_keepalive_cycle(socket: &UdpSocket, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
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
    let mut skipped_offline = 0;
    let mut skipped_tags = 0;

    // 2. Iterate Peers
    for (_node_key, peer) in status.peers {
        // --- FILTER: Offline ---
        // If 'Online' is missing (None), we assume false to be safe, or check JSON output behavior.
        // Usually, offline nodes explicitly say "Online": false.
        if !peer.online.unwrap_or(false) {
            skipped_offline += 1;
            continue;
        }

        // --- FILTER: Tags ---
        let peer_tags = peer.tags.as_deref().unwrap_or(&[]);

        // Check Inclusion (--with-tags)
        // If with_tags is set, the peer MUST have at least one of those tags.
        if let Some(required_tags) = &args.with_tags {
            let has_required = required_tags.iter().any(|t| peer_tags.contains(t));
            if !has_required {
                skipped_tags += 1;
                continue;
            }
        }

        // Check Exclusion (--without-tags)
        // If without_tags is set, the peer MUST NOT have any of those tags.
        if let Some(excluded_tags) = &args.without_tags {
            let has_excluded = excluded_tags.iter().any(|t| peer_tags.contains(t));
            if has_excluded {
                skipped_tags += 1;
                continue;
            }
        }

        // 3. Send Packets
        for ip_str in peer.tailscale_ips {
            // Parse string to IP Address
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                // ONLY process IPv4 addresses
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

    println!(
        "Keepalive cycle: Sent {} pkts | Skipped: {} offline, {} tag-filtered", 
        sent_count, skipped_offline, skipped_tags
    );
    Ok(())
}