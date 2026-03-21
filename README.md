# SSLProxyManager

**[中文文档 (Chinese)](README_zh.md)**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://tauri.app/)
[![Platforms](https://img.shields.io/badge/Platforms-Windows%20%7C%20Linux%20%7C%20macOS-green.svg)](#)

A powerful desktop proxy management application built with **Tauri 2 + Rust**, featuring an intuitive **Vue 3** frontend. Manage HTTP/HTTPS, WebSocket, and Stream proxies with comprehensive access control, rate limiting, observability, and testing tools—all in one unified interface.

## Highlights

- **Multi-Protocol Support**: HTTP/HTTPS reverse proxy, WebSocket (WS/WSS), and Stream (TCP/UDP Layer 4)
- **Access Control**: Fine-grained HTTP/WS/Stream switches with LAN allow, whitelist, and blacklist modes
- **Observability**: Real-time dashboard, historical metrics, request logs (SQLite), and system monitoring
- **Built-in Testing Tools**: HTTP tester, route matcher, performance tester, DNS lookup, SSL cert inspector, port scanner, and more
- **Performance Optimized**: LRU caching, connection pooling, zero-copy architecture, and Rustls for TLS

## Screenshots

https://github.com/user-attachments/assets/b41b3d38-19c5-4124-a439-c4c011c16a5b

![SystemMetrics](./screenshots/SystemMetrics.jpg)
![ScreenShot1](./screenshots/1.jpg)
![ScreenShot2](./screenshots/2.jpg)
![ScreenShot3](./screenshots/3.jpg)
![ScreenShot4](./screenshots/4.jpg)
![ScreenShot5](./screenshots/5.jpg)

## Core Features

### HTTP/HTTPS Proxy (`rules` / `routes`)

- Multiple listen addresses (`listen_addr` + `listen_addrs`)
- TLS certificates with Rustls
- Basic Auth with optional `Authorization` header forwarding
- Route matching by path + optional host/method/header constraints
- URL rewrite, request/response body replacement
- Static directory serving with SPA fallback
- Weighted upstreams (smooth weighted round-robin)
- Per-route follow-redirects

### WebSocket Proxy (`ws_proxy`)

- Global switch + per-rule enable/disable
- WS/WSS protocol support
- Longest-prefix path routing

### Stream Proxy (`stream`)

- TCP and UDP forwarding
- Upstream health check and failover
- Hash-based upstream selection (`$remote_addr`) with optional consistent hashing

### Access Control

- Independent switches for HTTP / WS / Stream
- LAN allow mode, whitelist, and blacklist

### Observability

- Real-time dashboard metrics
- Historical metrics and request logs (SQLite)
- System metrics (Linux/Windows): CPU, memory, swap, network, disk throughput, TCP states, process/file descriptor counts, uptime
- Real-time log panel

### Built-in Testing Tools

- HTTP request tester
- Route match tester + test suite
- Performance tester
- Configuration validator
- DNS lookup
- SSL certificate inspector
- Self-signed certificate generator
- Port scanner
- Encode/decode utilities

### Performance Features

- LRU caching for upstream responses
- Connection pooling with configurable idle timeout
- HTTP/2 support
- Gzip and Brotli compression
- Zero-copy file serving
- Efficient buffer pool management

## Tech Stack

| Layer | Technologies |
|-------|-------------|
| Backend | Rust, Tauri 2, Axum, Tokio, SQLx (SQLite) |
| Frontend | Vue 3, Vite, Element Plus, ECharts, Vue I18n |
| TLS | Rustls (memory-safe TLS) |
| Platform | Windows, Linux, macOS |

## Quick Start

### Prerequisites

- Node.js + npm
- Rust stable toolchain

### Install & Run

```bash
# Install frontend dependencies
cd frontend && npm install && cd ..

# Development mode
npm run tauri:dev

# Build for release
npm run tauri:build
```

## Project Structure

```
SSLProxyManager/
├── src/                    # Rust backend
│   ├── proxy.rs           # HTTP/HTTPS proxy core
│   ├── ws_proxy.rs        # WebSocket proxy
│   ├── stream_proxy.rs    # Stream (TCP/UDP) proxy
│   ├── access_control.rs  # Access control
│   ├── config.rs          # Configuration
│   └── ...
├── frontend/              # Vue 3 frontend
│   ├── src/
│   │   ├── components/    # Vue components
│   │   ├── i18n/         # Internationalization
│   │   └── ...
│   └── ...
├── config.toml.example    # Configuration template
└── tauri.conf.json       # Tauri settings
```

## Configuration

### Config File Locations

Runtime config is TOML format.

| Platform | Location |
|----------|----------|
| Debug mode | `./config.toml` in project root (if exists) |
| Linux | `$XDG_CONFIG_HOME/SSLProxyManager/config.toml` or `~/.config/SSLProxyManager/config.toml` |
| Windows | Next to executable, or `%APPDATA%\SSLProxyManager\config.toml` |
| macOS | `~/Library/Application Support/SSLProxyManager/config.toml` |

### Quick Config Reference

Use `config.toml.example` as the main template.

#### HTTP/HTTPS (`[[rules]]`)

```toml
listen_addrs = ["0.0.0.0:8080"]
ssl_enable = true
cert_file = "/path/to/cert.pem"
key_file = "/path/to/key.pem"
basic_auth_enable = true
basic_auth_username = "admin"
basic_auth_password = "secret"

[[rules.routes]]
path = "/api"
host = "example.com"  # optional
methods = ["GET", "POST"]  # optional
upstream_url = "http://localhost:3000"
weight = 1
```

#### WebSocket (`[[ws_proxy]]`)

```toml
ws_proxy_enabled = true

[[ws_proxy.rules]]
enabled = true
listen_addr = "0.0.0.0:8081"
ssl_enable = false

[[ws_proxy.rules.routes]]
path = "/ws"
upstream_url = "ws://localhost:8082"
```

#### Stream (`[stream]`)

```toml
[stream]
enabled = true

[[stream.upstreams]]
name = "backend"
hash_key = "$remote_addr"
consistent = true

[[stream.upstreams.servers]]
addr = "10.0.0.1:80"
weight = 1
max_fails = 3
fail_timeout = "30s"

[[stream.servers]]
enabled = true
listen_port = 9000
proxy_pass = "backend"
```

#### System Metrics

```toml
system_metrics_sample_interval_secs = 10  # 1-300 seconds
system_metrics_persistence_enabled = true
```

#### Metrics Storage

```toml
[metrics_storage]
enabled = true
db_path = "./data/metrics.db"
```

## CI: Manual Build

Workflow: `.github/workflows/manual-build-single-platform.yml`

### Trigger

GitHub Actions → `Manual Build (Single Platform)` → `Run workflow`

### Inputs

| Input | Options | Description |
|-------|---------|-------------|
| `platform` | `windows-x64`, `linux-amd64`, `macos-arm64`, `macos-x64` | Target platform |
| `publish_release` | `true`, `false` | Upload to GitHub Release? |
| `release_tag` | (optional) | Override version tag |

## FAQ

**Q: Which port is used in dev mode?**  
A: `5173` (configured in `tauri.conf.json -> build.devUrl`)

**Q: How to customize frontend dev/build commands?**  
A: Edit `tauri.conf.json` → `build.beforeDevCommand` and `build.beforeBuildCommand`

**Q: Can the app run in tray mode?**  
A: Yes. Closing the window hides to tray instead of exiting.

**Q: How to enable metrics persistence?**  
A: Set `metrics_storage.enabled = true` and `metrics_storage.db_path` in your config.

**Q: Does it support HTTP/2?**  
A: Yes, HTTP/2 is enabled by default (`enable_http2 = true`).

## Disclaimer

This project is for **learning and legal, compliant network proxy/reverse proxy configuration management** scenarios only.

- **Legal Compliance**: Ensure your use complies with local laws. Do not use for unauthorized access, attacks, data theft, or any illegal purposes.
- **No Warranty**: Provided "as is" without warranties.
- **Limitation of Liability**: No responsibility for direct or indirect losses.

If you do not agree, please do not use this project.

## License

MIT. See [LICENSE](LICENSE).

## Repository

- GitHub: <https://github.com/userfhy/SSLProxyManager-Tauri>
