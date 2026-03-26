# proxy 模块说明

本目录负责 HTTP 反向代理主链路，目标是把一次请求拆成可维护的阶段：

1. 入站上下文构建
2. 路由匹配与前置校验
3. 请求改写并转发到上游
4. 响应处理与回写
5. 结构化日志与运行态控制

---

## 目录职责

- `mod.rs`
  - `proxy` 域模块装配
  - 暴露 HTTP 主链路入口 `proxy_handler`
  - 串联 dispatch/request/response/static_files 等阶段
- `context.rs`
  - 构建请求上下文（客户端、方法、URI、headers 等）
- `dispatch.rs`
  - 路由选择与 guard 执行（权限、方法、条件匹配）
- `matching.rs`
  - host/path/method 等匹配细节与辅助函数
- `auth.rs`
  - 鉴权相关逻辑（如 basic auth、healthz）
- `request.rs`
  - 代理请求构建：URL 改写、header 处理、body 准备
- `upstream.rs`
  - 上游 URL/路由拼装与 upstream 相关辅助
- `response.rs`
  - 上游响应处理：状态、header、body、压缩/替换等回写策略
- `early.rs`
  - 请求早期拦截逻辑（在完整代理流程前快速返回）
- `static_files.rs`
  - 静态文件路径分流与响应
- `logging.rs`
  - 请求日志写入/读取/清理接口
- `runtime.rs`
  - 统一运行时控制入口（启动/停止/状态）
  - 编排 HTTP、WebSocket、TCP/UDP stream 三类监听器
- `server.rs`
  - HTTP/HTTPS server 层编排与监听相关集成
- `listen.rs`
  - 监听地址解析
- `helpers.rs`
  - 通用工具（content-type/cache/regex 等）
- `lifecycle.rs`
  - 生命周期辅助逻辑
- `types.rs`
  - proxy 域核心类型定义（含共享状态）
- `ws_proxy.rs`
  - WebSocket 代理运行时
  - WS 监听、升级、上游转发与访问控制
- `stream_proxy.rs`
  - TCP/UDP stream 代理运行时
  - stream 配置校验、监听、上游选择与连接转发

---

## 请求处理主流程（简化）

`proxy_handler`（`mod.rs`）核心路径：

1. `RequestContext::new(...)`
2. `resolve_route_and_run_guards(...)`
3. 如果命中静态目录且允许，走 `serve_static_owned(...)`
4. `prepare_proxy_request(...)` 构造上游请求
5. `client.execute(...)` 请求上游
6. `handle_upstream_response(...)` 统一响应回写

这条链路是后续优化和排障的主观察点。

---

## 维护建议

- 新增功能优先放在子模块，不要把 `mod.rs` 变回“巨文件”。
- 与“路由匹配”相关的改动尽量只动 `matching.rs` + `dispatch.rs`。
- 与“协议改写”相关的改动尽量只动 `request.rs` / `response.rs`。
- 性能问题先看：
  1) 匹配阶段（`matching/dispatch`）
  2) body 处理（`request/response`）
  3) 上游连接与重试策略（`upstream/runtime/server`）
- 新增公共辅助时，优先放 `helpers.rs`，避免循环依赖。

---

## 与其他模块关系

- 依赖 `config` 提供路由与行为配置
- 依赖 `metrics` 进行请求日志与统计落库
- 与 `system_metrics` 相互独立（一个是代理请求指标，一个是系统资源指标）
- `runtime.rs` 作为聚合层，负责调用 `ws_proxy.rs` 与 `stream_proxy.rs`
