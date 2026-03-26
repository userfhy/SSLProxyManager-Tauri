# `src/` Module Guide

This file documents backend module layout and naming conventions for `SSLProxyManager`.

## Current Assessment

Overall naming and structure are **reasonable and consistent**:

- Domain modules use folder + `mod.rs` pattern (`proxy/`, `metrics/`, `system_metrics/`, `commands/`)
- File names are snake_case and mostly behavior-oriented (`matching`, `dispatch`, `lifecycle`, `sampler`, `writer`, `query`)
- Platform-specific collectors are clearly split (`collect/linux.rs`, `collect/windows.rs`)
- Tauri command binding layer is isolated under `commands/*`

## Top-Level Modules (quick map)

- `main.rs`: Tauri entry, command registration, lifecycle hooks
- `app.rs`: app bootstrap / cleanup orchestration
- `config.rs`: config models, loading/saving, validation helpers
- `proxy/`: HTTP/HTTPS reverse proxy pipeline
- `proxy/ws_proxy.rs`: WebSocket proxy runtime
- `proxy/stream_proxy.rs`: TCP/UDP stream proxy runtime
- `metrics/`: request metrics + SQLite persistence + aggregation
- `system_metrics/`: host metrics sampling/persistence/query
- `commands/`: invoke command facade for frontend
- `access_control.rs`: ACL/whitelist/blacklist decisions
- `rate_limit.rs`: per-listen address rate limiting
- `single_instance.rs`: second-instance activation handler (focus existing main window)
- `tray.rs`: tray integration
- `update.rs`: update check logic
- `test_tools.rs`: built-in testing utilities backend

## Naming Convention (recommended)

- Prefer **domain noun** for module roots: `proxy`, `metrics`, `system_metrics`, `commands`
- Prefer **single-responsibility behavior files** inside domain modules: `query`, `writer`, `service`, `state`
- Keep platform split under a dedicated folder: `collect/linux.rs`, `collect/windows.rs`
- Avoid mixed naming style in one layer (e.g. `xxx_tool.rs` and `tools.rs` side by side)

## Low-Risk Cleanup Candidates (optional)

These are optional improvements; current code is valid.

1. `single_instance.rs`
   - If still unused, remove or wire into actual startup path to avoid stale module confusion.
2. `network_optimizer.rs` and `cache_optimizer.rs`
   - Consider adding a short header doc comment in each file explaining scope and call sites.

## Suggested Future Rule for New Files

When adding files under `src/`, use this order of preference:

1. Place under existing domain module if related
2. Name by responsibility (`query`, `writer`, `service`, `state`, `helpers`)
3. If cross-domain utility, keep at top-level and add a short module doc comment
4. Update this README when introducing a new top-level module
