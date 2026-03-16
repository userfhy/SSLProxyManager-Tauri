# SSLProxyManager

**[中文文档 (Chinese)](README_zh.md)**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

SSLProxyManager is a desktop proxy management app built with **Tauri 2 + Rust** (frontend: **Vue 3 + Vite + Element Plus**).
It provides one UI to manage:

- HTTP/HTTPS reverse proxy
- WebSocket (WS/WSS) reverse proxy
- Stream (TCP/UDP, Layer 4) proxy
- Access control, rate limiting, logs, metrics, and test tools

## Core Features

- HTTP/HTTPS proxy (`rules` / `routes`)
  - Multiple listen addresses (`listen_addr` + `listen_addrs`)
  - TLS certificates
  - Basic Auth (with optional forwarding of `Authorization`)
  - Route matching by path + optional host/method/header constraints
  - URL rewrite, request/response body replacement
  - Static directory serving with SPA fallback
  - Weighted upstreams (smooth weighted round-robin)
  - Optional follow-redirects per route
- WebSocket proxy (`ws_proxy`)
  - Global switch + per-rule enable
  - WS/WSS support
  - Longest-prefix path routing
- Stream proxy (`stream`)
  - TCP and UDP proxy
  - Upstream health/failover support
  - Hash-based upstream selection (`$remote_addr`) with optional consistent behavior
- Access control
  - HTTP / WS / Stream switches
  - LAN allow mode, whitelist, blacklist
- Observability
  - Real-time dashboard metrics
  - Historical metrics and request logs (SQLite)
  - Real-time log panel
- Utility tools in UI
  - HTTP request test
  - Route match test + route test suite
  - Built-in performance test
  - Config validator
  - DNS lookup
  - SSL cert inspection
  - Self-signed cert generation
  - Port scan
  - Encode/decode tools

## Tech Stack

- Backend: Rust, Tauri 2, Axum, Tokio, SQLx (SQLite)
- Frontend: Vue 3, Vite, Element Plus, ECharts, Vue I18n
- TLS: Rustls

## Screenshots

[![Preview Video](./SSLProxyManager_20260316.mp4)

![ScreenShot1](./screenshots/1.jpg)
![ScreenShot2](./screenshots/2.jpg)
![ScreenShot3](./screenshots/3.jpg)
![ScreenShot4](./screenshots/4.jpg)
![ScreenShot5](./screenshots/5.jpg)

## Project Layout

- `src/`: Rust backend
- `frontend/`: frontend app
- `config.toml.example`: config reference
- `tauri.conf.json`: Tauri build/dev settings

## Requirements

- Node.js + npm
- Rust stable toolchain

## Local Development

Install frontend dependencies:

```bash
cd frontend
npm install
```

Run in project root:

```bash
npm run tauri:dev
```

Build release package:

```bash
npm run tauri:build
```

## Configuration File Location

Runtime config is TOML.

- Debug mode: if `./config.toml` exists in project root, it is preferred.
- Linux: `$XDG_CONFIG_HOME/SSLProxyManager/config.toml` or `~/.config/SSLProxyManager/config.toml`
- Windows:
  - Prefer `config.toml` next to executable
  - Fallback `%APPDATA%\SSLProxyManager\config.toml`
- macOS: `~/Library/Application Support/SSLProxyManager/config.toml`

## Configuration Guide

Use `config.toml.example` as the main template.

### HTTP/HTTPS (`[[rules]]`)

- `listen_addr`: legacy single address (for compatibility)
- `listen_addrs`: preferred multiple addresses; if empty, fallback to `listen_addr`
- `ssl_enable`, `cert_file`, `key_file`
- `basic_auth_enable`, `basic_auth_username`, `basic_auth_password`, `basic_auth_forward_header`
- `rate_limit_*` fields (optional)

`[[rules.routes]]` supports:

- `path`, `host`, `methods`, `headers`
- `proxy_pass_path`, `follow_redirects`
- `url_rewrite_rules`
- `request_body_replace`, `response_body_replace` (optional `content_types` filter)
- `set_headers`, `remove_headers`
- `static_dir` (served before upstream)
- `[[rules.routes.upstreams]]` with `url` + `weight`

### WebSocket (`[[ws_proxy]]`)

- Global switch: `ws_proxy_enabled`
- Rule fields: `enabled`, `listen_addr`, `ssl_enable`, `cert_file`, `key_file`
- Routes: `[[ws_proxy.routes]]` with `path` and `upstream_url`

Note: WS rule uses `listen_addr` (not `listen_addrs`).

### Stream (`[stream]`)

- `stream.enabled`
- `[[stream.upstreams]]`: `name`, `hash_key`, `consistent`
- `[[stream.upstreams.servers]]`: `addr`, `weight`, `max_fails`, `fail_timeout`
- `[[stream.servers]]`: `enabled`, `listen_port`, `listen_addr` (optional), `udp`, `proxy_pass`, `proxy_connect_timeout`, `proxy_timeout`

### Global Flags (Generated Default Config)

Defaults below are from current backend code (`src/config.rs`) when creating a new config file:

- `ws_proxy_enabled = true`
- `http_access_control_enabled = true`
- `ws_access_control_enabled = true`
- `stream_access_control_enabled = true`
- `allow_all_lan = true`
- `allow_all_ip = false`
- `auto_start = false`
- `show_realtime_logs = true`
- `realtime_logs_only_errors = false`
- `stream_proxy = true` (legacy field)
- `max_body_size = 10485760`
- `max_response_body_size = 10485760`
- `upstream_connect_timeout_ms = 3000`
- `upstream_read_timeout_ms = 30000`
- `upstream_pool_max_idle = 200`
- `upstream_pool_idle_timeout_sec = 90`
- `enable_http2 = true`
- `compression_enabled = false`
- `compression_gzip = true`
- `compression_brotli = true`
- `compression_min_length = 1024`
- `compression_gzip_level = 6`
- `compression_brotli_level = 6`

Notes:

- `stream_proxy` is legacy and kept for compatibility. Prefer `[stream].enabled`.
- `config.toml.example` may use sample values different from generated defaults.

## FAQ

- Which port is used in dev mode?
  - `5173` (`tauri.conf.json -> build.devUrl`).
- How to customize frontend dev/build commands?
  - Edit `tauri.conf.json` `build.beforeDevCommand` and `build.beforeBuildCommand`.
- Can the app run in tray mode?
  - Yes. Closing the window hides to tray instead of exiting.

## Disclaimer

This project is for learning and legal, compliant network proxy/reverse proxy configuration management scenarios only. Use of this software may involve network access control, certificate management, traffic forwarding, and other operations, with potential risks including but not limited to data leakage, service interruption, configuration errors leading to security risks, etc. You are responsible for evaluating and assuming all risks and responsibilities when using this project.

- **Legal Compliance**: Please ensure your use complies with local laws and regulations and relevant network service terms. It is prohibited to use this project for any unauthorized penetration, attacks, bypassing access controls, data theft, spreading malware, infringing on others' privacy, or any other illegal or unauthorized purposes. Any legal liability, administrative penalties, third-party claims, and related consequences arising from your use of this project for illegal, non-compliant, or unauthorized activities shall be borne by you, and the authors and contributors assume no responsibility.
- **No Warranty**: This project is provided "as is" without any express or implied warranty (including but not limited to fitness, reliability, accuracy, availability, error-free/defect-free, etc.).
- **Limitation of Liability**: The authors and contributors assume no responsibility for any direct or indirect losses (including but not limited to profit loss, data loss, business interruption, equipment or system damage, etc.) caused by the use or inability to use this project.

If you do not agree to the above terms, please do not use, distribute, or develop based on this project.

## License

MIT. See [LICENSE](LICENSE).

## Repository

- GitHub: <https://github.com/userfhy/SSLProxyManager-Tauri>
