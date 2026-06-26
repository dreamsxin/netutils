# netutils — Network Diagnostic Toolkit

English | [中文](README.md)

---

A cross-platform command-line network diagnostic tool written in Rust. Covers network interfaces, routing, egress detection, proxy detection, Ping, DNS, Traceroute, port scanning, connectivity testing, connection listing, one-click diagnostics, and full-link diagnostics.

### Features

| Command | Description | Example |
|---------|-------------|---------|
| `(none)` | Show all network info | `netutils` |
| `iface` | Network interface list | `netutils iface` |
| `egress` | Traffic egress + routing logic | `netutils egress` |
| `route` | Routing table | `netutils route` |
| `proxy` | Proxy settings | `netutils proxy` |
| `ping` | Ping host (ICMP/TCP) | `netutils ping google.com --count 4` |
| `dns` | DNS query | `netutils dns example.com --type mx` |
| `trace` | Traceroute | `netutils trace google.com` |
| `scan` | Port scan | `netutils scan 192.168.1.1 80,443` |
| `check` | Connectivity test | `netutils check https://example.com` |
| `connections` | Network connections (TCP/UDP) | `netutils connections --state LISTEN` |
| `diag` | One-click diagnostics | `netutils diag` |
| `diagnose` | Full-link diagnostics (DNS→Ping→TCP→HTTPS→Trace) | `netutils diagnose example.com` |

### Installation

```bash
# Install from crates.io (recommended)
cargo install netutils-cli

# Use directly after install
netutils --help
```

### Quick Start

```bash
# Build from source
git clone https://github.com/dreamsxin/netutils-cli.git
cd netutils-cli
cargo build --release

# Run
./target/release/netutils

# Help
./target/release/netutils --help
```

### One-Click Diagnostics

```bash
$ netutils diag

🔍 Network Diagnostics  2026-06-25 14:30:00

  ✅ [Egress] Network connected (egress: Ethernet 192.168.50.4)
  ✅ [Domestic DNS] DNS OK (baidu.com → 111.63.65.247, 45ms)
  ✅ [Global DNS] DNS OK (google.com → 142.250.69.174, 180ms)
  ✅ [Gateway] Gateway reachable (192.168.50.1, 0.5ms)
  ⚠️  [Proxy] System proxy enabled (127.0.0.1:7897)
  ✅ [Domestic HTTP] HTTPS OK (baidu.com → 200, 54ms) [via proxy]
  ✅ [Global HTTP] HTTPS OK (google.com → 200, 1096ms) [via proxy]
  ❌ [IPv6] IPv6 unavailable

  Time: 8.2s
```

### Full-Link Diagnostics

Automatically runs a complete link check (DNS → Ping → TCP → HTTPS → Traceroute) on a target host and pinpoints the failure:

```bash
$ netutils diagnose google.com

🔍 Link Diagnostics: google.com

  ✅ [① DNS Resolution]
     System DNS: google.com → 142.251.188.138 (199ms)
  ❌ [② Ping Probe]
     173.194.43.139 unreachable (100% loss)
  ❌ [③ TCP Port 443]
     Connection failed: timeout (3s)
  ✅ [④ HTTPS Request]
     https://google.com → 200 (807ms) [via proxy]
  ⚠️  [⑤ Traceroute (max 10 hops)]
     Not reached (10 hops)

  📍 Conclusion: Host unreachable
  Chain: ✅ DNS → ❌ Ping → ❌ TCP → ✅ HTTPS

  Time: 20.2s
```

Auto-conclusion: DNS fail → "DNS resolution failed" / Ping fail → "Host unreachable" / TCP fail → "TCP port unreachable" / HTTPS fail → "HTTPS failed" / All OK → "Link healthy"

### Key Features

- **i18n**: Auto-detects system language (Chinese/English), `--lang zh|en` to override
- **JSON output**: `--json` flag for all commands, pipe-friendly
- **Color highlighting**: Egress in green, errors in red, virtual adapters in yellow
- **Command aliases**: `i`/`e`/`r`/`p`/`pg`/`d`/`t`/`s`/`c`/`co`/`dx`/`dg`
- **Cross-platform**: Windows (PowerShell), Linux (`ip`), macOS (`ifconfig`)
- **System proxy aware**: HTTP checks auto-detect and use system proxy, labeled `[via proxy]`/`[direct]`
- **Egress detection**: UDP probe identifies actual traffic egress + explains routing logic
- **Port range syntax**: `netutils scan host 80-100,443,8080-8090`

### Project Structure

```
netutils/
├── Cargo.toml
├── README.md           # Chinese
├── README_EN.md        # English (this file)
└── src/
    ├── main.rs              # Entry: CLI dispatch
    ├── cli.rs               # Subcommand definitions (clap)
    ├── i18n.rs              # Internationalization
    ├── table.rs             # Table rendering (unicode-width)
    ├── output.rs            # Output mode (Table/JSON)
    ├── util.rs              # Shared utilities
    ├── info/                # Network info detection
    │   ├── mod.rs           #   Orchestrator
    │   ├── interface.rs     #   Interface types + classification
    │   ├── interface_win.rs #   Windows (PowerShell)
    │   ├── interface_unix.rs#   Linux/macOS
    │   ├── route.rs         #   Route structures
    │   ├── route_win.rs     #   Windows routes
    │   ├── route_unix.rs    #   Linux/macOS routes
    │   ├── egress.rs        #   Egress detection (UDP probe)
    │   └── proxy.rs         #   Proxy detection
    ├── ping/mod.rs          # Ping (ICMP/TCP)
    ├── dns/mod.rs           # DNS query
    ├── traceroute/mod.rs    # Traceroute
    ├── portscan/mod.rs      # Port scan
    ├── connectivity/mod.rs  # Connectivity test
    ├── connections/mod.rs   # Connection listing
    ├── diag/mod.rs          # One-click diagnostics
    └── diagnose/mod.rs      # Full-link diagnostics
```

### Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI parsing |
| `tokio` | Async runtime |
| `surge-ping` | ICMP ping |
| `trust-dns-resolver` | DNS queries |
| `socket2` | Raw sockets (traceroute) |
| `reqwest` | HTTP connectivity |
| `serde` / `serde_json` | JSON output |
| `colored` | Terminal colors |
| `unicode-width` | CJK table alignment |
| `anyhow` | Error handling |
| `winreg` (Windows) | Registry proxy settings |

### License

MIT
