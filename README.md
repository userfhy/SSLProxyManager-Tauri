# SSLProxyManager

**[中文文档 (Chinese Documentation)](README_zh.md)**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A desktop proxy management tool with a modern web-based UI for managing HTTP/HTTPS, WebSocket, and Stream (TCP/UDP) reverse proxies.

SSLProxyManager is based on **Tauri 2 + Rust**, providing a management interface (frontend: **Vue 3 + Vite + Element Plus**) for configuring and managing:

- HTTP/HTTPS reverse proxy
- WebSocket (WS/WSS) reverse proxy
- Stream (TCP/UDP) Layer 4 proxy
- Static resource hosting
- Access control (LAN/whitelist/blacklist)
- Rate limiting
- Metrics storage and query
- Request logs and historical data
- Runtime status and log viewing
- Dashboard with real-time statistics

## Features Overview

- **HTTP/HTTPS Proxy (rules/routes)**
  - Multiple listen nodes (`listen_addr` / `listen_addrs`)
  - TLS (certificate/private key)
  - Basic Auth (with optional header forwarding)
  - Routing: path prefix matching + optional conditions (Host / HTTP methods / request headers)
  - Request/response body replacement supports optional `content_types` filtering (by `Content-Type`)
  - Upstream list (with weights)
  - `proxy_pass_path` path rewriting
  - Static directory priority (`static_dir`)
  - Header injection (`set_headers`)
  - Follow redirects configuration
  - HTTP/2 support (optional)
  - Compression (gzip/brotli)

- **WebSocket Proxy (ws_proxy)**
  - Each WS rule can be independently enabled
  - WS global switch `ws_proxy_enabled` (when globally disabled, WS listeners will not start)
  - TLS support (WSS)
  - Path-based routing

- **Stream Proxy (TCP/UDP, stream)**
  - `listen_port` listen port (TCP or UDP)
  - `proxy_pass` binds to upstream name
  - Upstream supports consistent selection by client IP (default `hash_key = "$remote_addr"`)
  - `proxy_connect_timeout` / `proxy_timeout` (string format, e.g., `300s`)

- **Access Control**
  - IP whitelist/blacklist
  - LAN access control (allow all LAN)
  - Separate controls for HTTP, WS, and Stream proxies

- **Metrics & Monitoring**
  - Real-time metrics collection
  - Historical metrics storage (SQLite)
  - Request logs with filtering and query
  - Dashboard with statistics and charts
  - Real-time log viewer

- **Application Features**
  - System tray integration
  - Auto-start on system boot
  - Single instance mode
  - Auto-update check
  - Internationalization (English/Chinese)
  - Dark/Light theme support

## Tech Stack

- **Backend**: Rust (Tauri 2), Axum, Tokio, SQLx
- **Frontend**: Vue 3, Vite, Element Plus, ECharts, Vue I18n
- **Key Libraries**: 
  - HTTP/WebSocket: Axum, Hyper, Tokio-Tungstenite
  - TLS: Rustls
  - Database: SQLite (via SQLx)
  - Configuration: TOML

## Screenshots

![ScreenShot1](./screenshots/1.jpg)

![ScreenShot2](./screenshots/2.jpg)

![ScreenShot3](./screenshots/3.jpg)

![ScreenShot4](./screenshots/4.jpg)

![ScreenShot5](./screenshots/5.jpg)

## Directory Structure

- `src/`: Rust backend code
- `frontend/`: Frontend project (Vite)
- `tauri.conf.json`: Tauri configuration (dev/build commands, devUrl, frontendDist, etc.)
- `config.toml`: Runtime configuration (can be placed in project root in development mode)
- `config.toml.example`: Configuration example

## Requirements

- Node.js + npm
- Rust toolchain (stable)

## Local Development

### 1) Install Frontend Dependencies

```bash
cd frontend
npm install
```

### 2) Start Tauri Development Mode

Execute in the project root directory:

```bash
npm run tauri:dev
```

This command will, according to `tauri.conf.json`:

- First enter `frontend` and execute `npm run dev`
- Then start Tauri and load `http://localhost:5173`

## Build & Release

Execute in the project root directory:

```bash
npm run tauri:build
```

This command will:

- First enter `frontend` and execute `npm run build` (output: `frontend/dist`)
- Then package with Tauri

## Configuration (config.toml)

The project uses TOML for configuration.

- **Development mode** (debug): If `config.toml` exists in the project root, it will be read with priority.
- **Linux production mode**: Default location `~/.config/SSLProxyManager/config.toml`

> It is recommended to refer directly to `config.toml.example`.

### 1) HTTP/HTTPS Proxy (rules)

- `[[rules]]`: Listen node
  - `listen_addr`: Legacy single listen address (kept for backward compatibility)
  - `listen_addrs`: Preferred multiple listen addresses, e.g. `[":8888", ":8889"]` (if empty, falls back to `listen_addr`)
  - `ssl_enable`: Whether to enable TLS
  - `cert_file` / `key_file`: Certificate and private key paths
  - `basic_auth_enable` / `basic_auth_username` / `basic_auth_password`
  - `basic_auth_forward_header`: Whether to forward the `Authorization` header to upstream
  - `routes`: Routes list
  - `ssl_enable`: Whether to enable TLS
  - `cert_file` / `key_file`: Certificate and private key paths
  - `basic_auth_enable` / `basic_auth_username` / `basic_auth_password`
- `[[rules.routes]]`: Route
  - `path`: Path prefix matching
  - `host`: Optional host constraint (supports exact match and wildcard like `*.example.com`)
  - `methods`: Optional HTTP method constraint (e.g. `["GET","POST"]`)
  - `headers`: Optional request header constraint (exact match; supports wildcard `*` in expected value)
  - `static_dir`: Static directory (optional)
  - `proxy_pass_path`: Forward path rewriting (optional)
  - `exclude_basic_auth`: Whether this route skips Basic Auth (optional)
  - `follow_redirects`: Whether the proxy follows upstream 30x redirects (optional)
  - `[rules.routes.set_headers]`: Header injection (optional)
  - `request_body_replace` / `response_body_replace`: Body replacement rules (optional)
    - `content_types`: Optional Content-Type filter for this replace rule (comma-separated, e.g. `text/html,application/json`)
  - `[[rules.routes.upstreams]]`: Upstream list (optional)

### 2) WS Proxy (ws_proxy)

- **`ws_proxy_enabled`**: WS global switch (default `true`)
  - `false`: WS listeners will not start (even if a ws rule has enabled=true)
  - `true`: Then each ws rule's `enabled` takes effect

- `[[ws_proxy]]`: WS listen rule list
  - `enabled`: Whether to enable this rule
  - `listen_addr`: Listen address, e.g., `0.0.0.0:8800`
  - `ssl_enable`: Whether to enable TLS (wss)
  - `cert_file` / `key_file`: Certificate and private key paths
  - `[[ws_proxy.routes]]`
    - `path`: Path prefix
    - `upstream_url`: Upstream WS address, e.g., `ws://127.0.0.1:9000`

### 3) Stream (TCP/UDP) Proxy (stream)

Stream is used for Layer 4 proxy: listen on a TCP/UDP port and forward to upstream.

- `[stream]`
  - `enabled`: Global switch
  - `[[stream.upstreams]]`
    - `name`: Upstream name (referenced by `proxy_pass`)
    - `hash_key`: Default `$remote_addr` (consistently select upstream by client IP)
    - `consistent`: Currently reserved as a configuration item
    - `[[stream.upstreams.servers]]`
      - `addr`: `host:port`
      - `weight` / `max_fails` / `fail_timeout`: Fields reserved (can be enhanced in future strategies)
  - `[[stream.servers]]`
    - `enabled`: Whether to enable
    - `listen_port`: Listen port
    - `udp`: `false`=TCP, `true`=UDP
    - `proxy_pass`: Reference upstream's `name`
    - `proxy_connect_timeout`: e.g., `300s`
    - `proxy_timeout`: e.g., `600s`

#### Nginx Example Comparison

You can use the following Nginx stream configuration to understand the correspondence:

```nginx
stream {
    upstream sendimage {
        hash $remote_addr consistent;
        server 59.xx.xx.xx:8089 max_fails=1 fail_timeout=30s;
    }

    server {
        listen 50002;
        proxy_pass sendimage;
        proxy_connect_timeout 300s;
        proxy_timeout 600s;
    }
}
```

The equivalent configuration in this project can be found in the `[stream]` section of `config.toml.example`.

### 4) Global Configuration

- `ws_proxy_enabled`: Enable/disable WebSocket proxy globally (default `true`)
- `http_access_control_enabled`: Enable HTTP access control (default `true`)
- `ws_access_control_enabled`: Enable WebSocket access control (default `false`)
- `stream_access_control_enabled`: Enable Stream proxy access control (default `true`)
- `allow_all_lan`: Allow all LAN IPs (default `true`)
- `auto_start`: Auto-start proxy service on application launch (default `true`)
- `show_realtime_logs`: Show real-time logs in UI (default `false`)
- `realtime_logs_only_errors`: Show only error logs in real-time view (default `false`)
- `stream_proxy`: Legacy field (use `[stream].enabled` instead)
- `max_body_size`: Maximum request body size in bytes (default `10485760` = 10MB)
- `max_response_body_size`: Maximum response body size in bytes (default `10485760` = 10MB)
- `upstream_connect_timeout_ms`: Upstream connection timeout in milliseconds (default `5000`)
- `upstream_read_timeout_ms`: Upstream read timeout in milliseconds (default `30000`)
- `upstream_pool_max_idle`: Maximum idle connections in connection pool (default `100`)
- `upstream_pool_idle_timeout_sec`: Idle connection timeout in seconds (default `60`)
- `enable_http2`: Enable HTTP/2 support (default `false`)

### 5) Access Control (Whitelist)

- `[[whitelist]]`: IP whitelist entries
  - `ip`: IP address or CIDR notation (e.g., `127.0.0.1` or `192.168.1.0/24`)

### 6) Metrics Storage

- `[metrics_storage]`: Metrics storage configuration
  - `enabled`: Enable metrics storage (default `true`)
  - `db_path`: SQLite database file path (e.g., `/path/to/metrics.db`)

### 7) Update Configuration

- `[update]`: Auto-update configuration
  - `enabled`: Enable update checking (default `true`)
  - `server_url`: Update server URL (empty for default)
  - `auto_check`: Automatically check for updates (default `true`)
  - `timeout_ms`: Update check timeout in milliseconds (default `10000`)
  - `ignore_prerelease`: Ignore pre-release versions (default `true`)

## UI Features

The application provides a comprehensive web-based management interface:

- **Dashboard**: Real-time statistics, metrics charts, and service status
- **Base Configuration**: Global settings and proxy service controls
- **HTTP/HTTPS Proxy Config**: Configure reverse proxy rules and routes
- **WebSocket Proxy Config**: Configure WS/WSS proxy rules
- **Stream Proxy Config**: Configure TCP/UDP Layer 4 proxy
- **Access Control**: Manage IP whitelist/blacklist
- **Metrics Storage**: View and manage metrics database
- **Request Logs**: Query and filter historical request logs
- **Log Viewer**: Real-time log viewing with filtering
- **About**: Version information and update checking

## FAQ

- **Q: What port does the frontend development server use?**  
  A: The default port is `5173` (see `devUrl` in `tauri.conf.json`).

- **Q: How do I change the frontend dev/build commands?**  
  A: Modify `build.beforeDevCommand` / `build.beforeBuildCommand` in the root directory's `tauri.conf.json`.

- **Q: Where is the configuration file located?**  
  A: In development mode, if `config.toml` exists in the project root, it takes priority. In production (Linux), the default location is `~/.config/SSLProxyManager/config.toml`.

- **Q: How do I enable auto-start on system boot?**  
  A: Set `auto_start = true` in `config.toml`, and the application will automatically start the proxy service on launch.

- **Q: Can I hide the application to system tray?**  
  A: Yes, clicking the close button will hide the window to the system tray instead of exiting. You can quit from the tray menu.

- **Q: How do I view historical metrics?**  
  A: Enable metrics storage in the configuration, then use the "Metrics Storage" tab in the UI to query historical data.

- **Q: How do I configure access control?**  
  A: Use the "Access Control" tab to manage IP whitelist/blacklist, or edit `[[whitelist]]` entries in `config.toml`.

## Disclaimer

This project is for learning and legal, compliant network proxy/reverse proxy configuration management scenarios only. Use of this software may involve network access control, certificate management, traffic forwarding, and other operations, with potential risks including but not limited to data leakage, service interruption, configuration errors leading to security risks, etc. You are responsible for evaluating and assuming all risks and responsibilities when using this project.

- **Legal Compliance**: Please ensure your use complies with local laws and regulations and relevant network service terms. It is prohibited to use this project for any unauthorized penetration, attacks, bypassing access controls, data theft, spreading malware, infringing on others' privacy, or any other illegal or unauthorized purposes. Any legal liability, administrative penalties, third-party claims, and related consequences arising from your use of this project for illegal, non-compliant, or unauthorized activities shall be borne by you, and the authors and contributors assume no responsibility.
- **No Warranty**: This project is provided "as is" without any express or implied warranty (including but not limited to fitness, reliability, accuracy, availability, error-free/defect-free, etc.).
- **Limitation of Liability**: The authors and contributors assume no responsibility for any direct or indirect losses (including but not limited to profit loss, data loss, business interruption, equipment or system damage, etc.) caused by the use or inability to use this project.

If you do not agree to the above terms, please do not use, distribute, or develop based on this project.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Repository

- GitHub: [https://github.com/userfhy/SSLProxyManager-Tauri](https://github.com/userfhy/SSLProxyManager-Tauri)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
