# tailscale-keepalived

By Gemini 3.0 Pro.

```
Write a "tailscale-keepalived" program using Rust. It's basically a loop that do the below things periodically:

1. Run "tailscale status".
2. Send a (arbitrary) udp packet to each node private ip to keep the connection alive. Don't wait for reply. It's just best effort.
```

## Build

1. Install [cross](https://crates.io/crates/cross): `cargo install cross`. Note `cross` uses Docker.
2. Run `./build_amd64.sh` or `./build_mips.sh` to build Linux amd64 or mipsle (softfloat) binary.
