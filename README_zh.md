# SSLProxyManager

**[English Documentation](README.md)**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

SSLProxyManager 是一个基于 **Tauri 2 + Rust** 的桌面代理管理工具（前端：**Vue 3 + Vite + Element Plus**）。
它将以下能力集中到一个界面中：

- HTTP/HTTPS 反向代理
- WebSocket（WS/WSS）反向代理
- Stream（TCP/UDP）四层代理
- 访问控制、限流、日志、指标、测试工具

## 核心功能

- HTTP/HTTPS 代理（`rules` / `routes`）
  - 多监听地址（`listen_addr` / `listen_addrs`）
  - TLS 证书
  - Basic Auth（可选转发 `Authorization`）
  - 路由匹配：Path + Host/Method/Header 条件
  - URL 重写、请求体/响应体替换
  - 静态目录优先 + SPA 回退
  - 上游加权（平滑加权轮询）
  - 路由级跟随重定向
- WebSocket 代理（`ws_proxy`）
  - 全局开关 + 规则级开关
  - WS/WSS 支持
  - 最长前缀路径匹配
- Stream 代理（`stream`）
  - TCP / UDP 转发
  - 上游故障感知与回退
  - 基于 `$remote_addr` 的哈希选择与一致性策略
- 访问控制
  - HTTP / WS / Stream 独立开关
  - 局域网放行、白名单、黑名单
- 可观测性
  - 仪表板实时指标
  - SQLite 历史指标与请求日志
  - 实时日志面板
- 内置测试工具（UI）
  - HTTP 测试
  - 路由匹配测试 + 测试套件
  - 性能测试
  - 配置校验
  - DNS 查询
  - 证书信息查询
  - 自签证书生成
  - 端口扫描
  - 编解码工具

## 技术栈

- 后端：Rust、Tauri 2、Axum、Tokio、SQLx（SQLite）
- 前端：Vue 3、Vite、Element Plus、ECharts、Vue I18n
- TLS：Rustls

## 程序界面

[![Preview Video](./SSLProxyManager_20260316.mp4)

![ScreenShot1](./screenshots/1.jpg)
![ScreenShot2](./screenshots/2.jpg)
![ScreenShot3](./screenshots/3.jpg)
![ScreenShot4](./screenshots/4.jpg)
![ScreenShot5](./screenshots/5.jpg)

## 项目结构

- `src/`：Rust 后端
- `frontend/`：前端项目
- `config.toml.example`：配置参考模板
- `tauri.conf.json`：Tauri 开发与构建配置

## 环境要求

- Node.js + npm
- Rust stable 工具链

## 本地开发

安装前端依赖：

```bash
cd frontend
npm install
```

在项目根目录运行：

```bash
npm run tauri:dev
```

构建发布包：

```bash
npm run tauri:build
```

## 配置文件位置

运行配置使用 TOML。

- Debug 模式：若项目根目录存在 `./config.toml`，优先使用。
- Linux：`$XDG_CONFIG_HOME/SSLProxyManager/config.toml` 或 `~/.config/SSLProxyManager/config.toml`
- Windows：
  - 优先使用可执行文件同目录 `config.toml`
  - 否则使用 `%APPDATA%\SSLProxyManager\config.toml`
- macOS：`~/Library/Application Support/SSLProxyManager/config.toml`

## 配置说明

建议以 `config.toml.example` 为模板。

### HTTP/HTTPS（`[[rules]]`）

- `listen_addr`：兼容旧版的单地址
- `listen_addrs`：推荐的多地址；为空时回退到 `listen_addr`
- `ssl_enable`、`cert_file`、`key_file`
- `basic_auth_enable`、`basic_auth_username`、`basic_auth_password`、`basic_auth_forward_header`
- `rate_limit_*`（可选）

`[[rules.routes]]` 支持：

- `path`、`host`、`methods`、`headers`
- `proxy_pass_path`、`follow_redirects`
- `url_rewrite_rules`
- `request_body_replace`、`response_body_replace`（可选 `content_types`）
- `set_headers`、`remove_headers`
- `static_dir`（优先于 upstream）
- `[[rules.routes.upstreams]]` 的 `url` + `weight`

### WebSocket（`[[ws_proxy]]`）

- 全局开关：`ws_proxy_enabled`
- 规则字段：`enabled`、`listen_addr`、`ssl_enable`、`cert_file`、`key_file`
- 路由字段：`[[ws_proxy.routes]]` 下的 `path` 和 `upstream_url`

注意：WS 规则使用 `listen_addr`，不是 `listen_addrs`。

### Stream（`[stream]`）

- `stream.enabled`
- `[[stream.upstreams]]`：`name`、`hash_key`、`consistent`
- `[[stream.upstreams.servers]]`：`addr`、`weight`、`max_fails`、`fail_timeout`
- `[[stream.servers]]`：`enabled`、`listen_port`、`listen_addr`（可选）、`udp`、`proxy_pass`、`proxy_connect_timeout`、`proxy_timeout`

### 全局配置默认值（首次自动生成配置）

以下默认值来自当前后端代码 `src/config.rs`：

- `ws_proxy_enabled = true`
- `http_access_control_enabled = true`
- `ws_access_control_enabled = true`
- `stream_access_control_enabled = true`
- `allow_all_lan = true`
- `allow_all_ip = false`
- `auto_start = false`
- `show_realtime_logs = true`
- `realtime_logs_only_errors = false`
- `stream_proxy = true`（遗留字段）
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

说明：

- `stream_proxy` 为兼容字段，建议优先使用 `[stream].enabled`。
- `config.toml.example` 是示例配置，部分值可能与“自动生成默认值”不同。

## 常见问题

- 开发模式前端端口是多少？
  - `5173`（见 `tauri.conf.json -> build.devUrl`）。
- 如何改前端 dev/build 命令？
  - 修改 `tauri.conf.json` 的 `build.beforeDevCommand` 与 `build.beforeBuildCommand`。
- 关闭窗口是否会退出程序？
  - 不会。默认会隐藏到系统托盘。

## 免责声明

本项目仅用于学习与合法合规的网络代理/反向代理配置管理场景。使用本软件可能涉及网络访问控制、证书管理、流量转发等操作，存在但不限于数据泄露、服务中断、配置错误导致安全风险等潜在风险。你在使用本项目时需自行评估并承担全部风险与责任。

- **合法合规**：请确保你的使用行为符合当地法律法规及相关网络服务条款。禁止将本项目用于任何未授权的渗透、攻击、绕过访问控制、窃取数据、传播恶意软件、侵犯他人隐私或其他任何违法违规用途。任何因你使用本项目从事违法违规或未授权行为所产生的法律责任、行政处罚、第三方索赔及相关后果，均由你自行承担，作者与贡献者不承担任何责任。
- **无担保**：本项目按“现状”提供，不提供任何形式的明示或暗示担保（包括但不限于适用性、可靠性、准确性、可用性、无错误/无漏洞等）。
- **责任限制**：对于因使用或无法使用本项目导致的任何直接或间接损失（包括但不限于利润损失、数据丢失、业务中断、设备或系统损坏等），作者与贡献者不承担任何责任。

如果你不同意上述条款，请勿使用、分发或基于本项目进行二次开发。

## 许可证

MIT，详见 [LICENSE](LICENSE)。

## 仓库

- GitHub: <https://github.com/userfhy/SSLProxyManager-Tauri>
