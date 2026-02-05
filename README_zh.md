# SSLProxyManager

**[(English Documentation)](README.md)**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

一个桌面代理管理工具，提供现代化的基于 Web 的管理界面，用于管理 HTTP/HTTPS、WebSocket 和 Stream（TCP/UDP）反向代理。

SSLProxyManager 基于 **Tauri 2 + Rust**，提供管理界面（前端：**Vue 3 + Vite + Element Plus**），用于配置与管理：

- HTTP/HTTPS 反向代理
- WebSocket（WS/WSS）反向代理
- Stream（TCP/UDP）四层代理
- 静态资源托管
- 访问控制（局域网/白名单/黑名单）
- 速率限制
- 指标存储与查询
- 请求日志与历史数据
- 运行状态与日志查看
- 仪表板实时统计

## 功能概览

- **HTTP/HTTPS 代理（rules/routes）**
  - 多监听节点（`listen_addr` / `listen_addrs`）
  - TLS（证书/私钥）
  - Basic Auth（支持可选头部转发）
  - 路由：Path 前缀匹配 + 可选条件（Host / HTTP 方法 / 请求头）
  - 请求/响应体替换支持按 `Content-Type` 过滤（`content_types`）
  - Upstream 列表（权重）
  - `proxy_pass_path` 路径改写
  - 静态目录优先（`static_dir`）
  - Header 注入（`set_headers`）
  - 跟随重定向配置
  - HTTP/2 支持（可选）
  - 压缩支持（gzip/brotli）

- **WebSocket 代理（ws_proxy）**
  - 每条 WS 规则可独立启用
  - WS 全局开关 `ws_proxy_enabled`（全局禁用时，WS 监听不会启动）
  - TLS 支持（WSS）
  - 基于路径的路由

- **Stream 代理（TCP/UDP，stream）**
  - `listen_port` 监听端口（TCP 或 UDP）
  - `proxy_pass` 绑定 upstream 名称
  - upstream 支持按客户端 IP 进行一致性选择（默认 `hash_key = "$remote_addr"`）
  - `proxy_connect_timeout` / `proxy_timeout`（字符串形式，例：`300s`）

- **访问控制**
  - IP 白名单/黑名单
  - 局域网访问控制（允许所有局域网）
  - HTTP、WS、Stream 代理独立控制

- **指标与监控**
  - 实时指标收集
  - 历史指标存储（SQLite）
  - 请求日志过滤与查询
  - 仪表板统计与图表
  - 实时日志查看器

- **应用功能**
  - 系统托盘集成
  - 系统启动自动运行
  - 单实例模式
  - 自动更新检查
  - 国际化支持（英文/中文）
  - 深色/浅色主题支持

## 技术栈

- **后端**：Rust（Tauri 2）、Axum、Tokio、SQLx
- **前端**：Vue 3、Vite、Element Plus、ECharts、Vue I18n
- **核心库**：
  - HTTP/WebSocket：Axum、Hyper、Tokio-Tungstenite
  - TLS：Rustls
  - 数据库：SQLite（通过 SQLx）
  - 配置：TOML

## 程序界面

![ScreenShot1](./screenshots/1.jpg)

![ScreenShot2](./screenshots/2.jpg)

![ScreenShot3](./screenshots/3.jpg)

![ScreenShot4](./screenshots/4.jpg)

![ScreenShot5](./screenshots/5.jpg)

## 目录结构

- `src/`：Rust 后端代码
- `frontend/`：前端项目（Vite）
- `tauri.conf.json`：Tauri 配置（dev/build 命令、devUrl、frontendDist 等）
- `config.toml`：运行配置（开发模式下可放项目根目录）
- `config.toml.example`：配置示例

## 环境要求

- Node.js + npm
- Rust 工具链（stable）

## 本地开发

### 1) 安装前端依赖

```bash
cd frontend
npm install
```

### 2) 启动 Tauri 开发模式

在项目根目录执行：

```bash
npm run tauri:dev
```

该命令会根据 `tauri.conf.json`：

- 先进入 `frontend` 并执行 `npm run dev`
- 然后启动 Tauri 并加载 `http://localhost:5173`

## 构建发布

在项目根目录执行：

```bash
npm run tauri:build
```

该命令会：

- 先进入 `frontend` 并执行 `npm run build`（产物：`frontend/dist`）
- 再由 Tauri 进行打包

## 配置说明（config.toml）

项目使用 TOML 进行配置。

- **开发模式**（debug）：如果项目根目录存在 `config.toml`，优先读取它。
- **Linux 生产模式**：默认位置 `~/.config/SSLProxyManager/config.toml`

> 建议直接参考 `config.toml.example`。

### 1) HTTP/HTTPS 代理（rules）

- `[[rules]]`：监听节点
  - `listen_addr`：监听地址，例如 `:8888` 或 `0.0.0.0:1024`
  - `ssl_enable`：是否启用 TLS
  - `cert_file` / `key_file`：证书与私钥路径
  - `basic_auth_enable` / `basic_auth_username` / `basic_auth_password`
- `[[rules.routes]]`：路由
  - `path`：Path 前缀匹配
  - `static_dir`：静态目录（可选）
  - `proxy_pass_path`：转发路径改写（可选）
  - `exclude_basic_auth`：该路由是否跳过 Basic Auth（可选）
  - `follow_redirects`：代理端是否跟随上游 30x（可选）
  - `[rules.routes.set_headers]`：注入 Header（可选）
  - `[[rules.routes.upstreams]]`：上游列表（可选）

### 2) WS 代理（ws_proxy）

- **`ws_proxy_enabled`**：WS 全局开关（默认 `true`）
  - `false`：不会启动 WS 监听（即使某条 ws rule enabled=true）
  - `true`：再按每条 ws rule 的 `enabled` 生效

- `[[ws_proxy]]`：WS 监听规则列表
  - `enabled`：是否启用该规则
  - `listen_addr`：监听地址，例如 `0.0.0.0:8800`
  - `ssl_enable`：是否启用 TLS（wss）
  - `cert_file` / `key_file`：证书与私钥路径
  - `[[ws_proxy.routes]]`
    - `path`：Path 前缀
    - `upstream_url`：上游 WS 地址，例如 `ws://127.0.0.1:9000`

### 3) Stream（TCP/UDP）代理（stream）

Stream 用于四层代理：监听一个 TCP/UDP 端口并转发到上游。

- `[stream]`
  - `enabled`：全局开关
  - `[[stream.upstreams]]`
    - `name`：upstream 名称（供 `proxy_pass` 引用）
    - `hash_key`：默认 `$remote_addr`（按客户端 IP 稳定选择上游）
    - `consistent`：当前作为配置项保留
    - `[[stream.upstreams.servers]]`
      - `addr`：`host:port`
      - `weight` / `max_fails` / `fail_timeout`：字段保留（可在后续增强策略）
  - `[[stream.servers]]`
    - `enabled`：是否启用
    - `listen_port`：监听端口
    - `udp`：`false`=TCP，`true`=UDP
    - `proxy_pass`：引用 upstream 的 `name`
    - `proxy_connect_timeout`：例如 `300s`
    - `proxy_timeout`：例如 `600s`

#### Nginx 示例对照

你可以用下面的 Nginx stream 配置理解对应关系：

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

在本项目中等价配置可参考 `config.toml.example` 的 `[stream]` 片段。

### 4) 全局配置

- `ws_proxy_enabled`：全局启用/禁用 WebSocket 代理（默认 `true`）
- `http_access_control_enabled`：启用 HTTP 访问控制（默认 `true`）
- `ws_access_control_enabled`：启用 WebSocket 访问控制（默认 `false`）
- `stream_access_control_enabled`：启用 Stream 代理访问控制（默认 `true`）
- `allow_all_lan`：允许所有局域网 IP（默认 `true`）
- `auto_start`：应用启动时自动启动代理服务（默认 `true`）
- `show_realtime_logs`：在 UI 中显示实时日志（默认 `false`）
- `realtime_logs_only_errors`：实时日志视图仅显示错误（默认 `false`）
- `stream_proxy`：遗留字段（请使用 `[stream].enabled` 代替）
- `max_body_size`：最大请求体大小（字节，默认 `10485760` = 10MB）
- `max_response_body_size`：最大响应体大小（字节，默认 `10485760` = 10MB）
- `upstream_connect_timeout_ms`：上游连接超时（毫秒，默认 `5000`）
- `upstream_read_timeout_ms`：上游读取超时（毫秒，默认 `30000`）
- `upstream_pool_max_idle`：连接池最大空闲连接数（默认 `100`）
- `upstream_pool_idle_timeout_sec`：空闲连接超时（秒，默认 `60`）
- `enable_http2`：启用 HTTP/2 支持（默认 `false`）

### 5) 访问控制（白名单）

- `[[whitelist]]`：IP 白名单条目
  - `ip`：IP 地址或 CIDR 表示法（例如：`127.0.0.1` 或 `192.168.1.0/24`）

### 6) 指标存储

- `[metrics_storage]`：指标存储配置
  - `enabled`：启用指标存储（默认 `true`）
  - `db_path`：SQLite 数据库文件路径（例如：`/path/to/metrics.db`）

### 7) 更新配置

- `[update]`：自动更新配置
  - `enabled`：启用更新检查（默认 `true`）
  - `server_url`：更新服务器 URL（为空则使用默认）
  - `auto_check`：自动检查更新（默认 `true`）
  - `timeout_ms`：更新检查超时（毫秒，默认 `10000`）
  - `ignore_prerelease`：忽略预发布版本（默认 `true`）

## 界面功能

应用程序提供了全面的基于 Web 的管理界面：

- **仪表板**：实时统计、指标图表和服务状态
- **基础配置**：全局设置和代理服务控制
- **HTTP/HTTPS 代理配置**：配置反向代理规则和路由
- **WebSocket 代理配置**：配置 WS/WSS 代理规则
- **Stream 代理配置**：配置 TCP/UDP 四层代理
- **访问控制**：管理 IP 白名单/黑名单
- **指标存储**：查看和管理指标数据库
- **请求日志**：查询和过滤历史请求日志
- **日志查看器**：实时日志查看与过滤
- **关于**：版本信息和更新检查

## 常见问题

- **Q: 前端开发服务器使用什么端口？**  
  A: 默认端口是 `5173`（见 `tauri.conf.json` 的 `devUrl`）。

- **Q: 如何更改前端 dev/build 命令？**  
  A: 修改根目录 `tauri.conf.json` 的 `build.beforeDevCommand` / `build.beforeBuildCommand`。

- **Q: 配置文件位于哪里？**  
  A: 开发模式下，如果项目根目录存在 `config.toml`，则优先读取。生产模式（Linux）下，默认位置是 `~/.config/SSLProxyManager/config.toml`。

- **Q: 如何启用系统启动时自动运行？**  
  A: 在 `config.toml` 中设置 `auto_start = true`，应用启动时会自动启动代理服务。

- **Q: 可以将应用隐藏到系统托盘吗？**  
  A: 可以，点击关闭按钮会将窗口隐藏到系统托盘而不是退出。可以从托盘菜单退出。

- **Q: 如何查看历史指标？**  
  A: 在配置中启用指标存储，然后在 UI 中使用"指标存储"标签页查询历史数据。

- **Q: 如何配置访问控制？**  
  A: 使用"访问控制"标签页管理 IP 白名单/黑名单，或在 `config.toml` 中编辑 `[[whitelist]]` 条目。

## 免责声明

本项目仅用于学习与合法合规的网络代理/反向代理配置管理场景。使用本软件可能涉及网络访问控制、证书管理、流量转发等操作，存在但不限于数据泄露、服务中断、配置错误导致安全风险等潜在风险。你在使用本项目时需自行评估并承担全部风险与责任。

- **合法合规**：请确保你的使用行为符合当地法律法规及相关网络服务条款。禁止将本项目用于任何未授权的渗透、攻击、绕过访问控制、窃取数据、传播恶意软件、侵犯他人隐私或其他任何违法违规用途。任何因你使用本项目从事违法违规或未授权行为所产生的法律责任、行政处罚、第三方索赔及相关后果，均由你自行承担，作者与贡献者不承担任何责任。
- **无担保**：本项目按“现状”提供，不提供任何形式的明示或暗示担保（包括但不限于适用性、可靠性、准确性、可用性、无错误/无漏洞等）。
- **责任限制**：对于因使用或无法使用本项目导致的任何直接或间接损失（包括但不限于利润损失、数据丢失、业务中断、设备或系统损坏等），作者与贡献者不承担任何责任。

如果你不同意上述条款，请勿使用、分发或基于本项目进行二次开发。

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

## 仓库

- GitHub: [https://github.com/userfhy/SSLProxyManager-Tauri](https://github.com/userfhy/SSLProxyManager-Tauri)

## 贡献

欢迎贡献！请随时提交 Pull Request。
