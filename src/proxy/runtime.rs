use anyhow::Result;
use tauri::Emitter;
use tracing::{error, info};

use super::lifecycle::{Phase, ServerHandle, PROXY_STATE};
use super::listen::precheck_rule;
use super::logging::{init_log_task, send_log, send_log_with_app, LOG_TX};
use super::server::start_rule_server;
use crate::{config, stream_proxy, ws_proxy};

pub fn start_server(app: tauri::AppHandle) -> Result<()> {
    init_log_task(app.clone());

    let cfg = config::get_config();

    send_log("[WS] Listener startup");
    if !cfg.ws_proxy_enabled {
        send_log("[WS] Disabled");
    } else if let Err(e) = ws_proxy::start_ws_servers(app.clone()) {
        send_log(format!("[WS] Failed to start listener: {e}"));
    }

    {
        send_log("[STREAM] Listener startup");

        let stream_cfg = cfg.stream.clone();
        let app2 = app.clone();
        if !stream_cfg.enabled {
            send_log("[STREAM] Disabled");
        } else {
            tauri::async_runtime::spawn(async move {
                if let Err(e) = stream_proxy::start_stream_servers(app2.clone(), &stream_cfg).await
                {
                    send_log_with_app(&app2, format!("[STREAM] Failed to start listener: {e}"));
                }
            });
        }
    }

    send_log("[HTTP] Listener startup");

    let rules: Vec<_> = cfg.rules.into_iter().filter(|r| r.enabled).collect();

    let expected: usize = rules
        .iter()
        .map(|r| {
            let n = r
                .listen_addrs
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .count();
            if n == 0 {
                1
            } else {
                n
            }
        })
        .sum();

    let generation = {
        let mut state = PROXY_STATE.lock();
        if matches!(state.phase, Phase::Starting | Phase::Running) {
            return Ok(());
        }
        if expected == 0 {
            state.phase = Phase::Stopped;
            drop(state);
            let _ = app.emit("status", "stopped");
            send_log("No listen rules configured; service remains stopped");
            return Ok(());
        }
        state.phase = Phase::Starting;
        state.expected = expected;
        state.started = 0;
        state.generation = state.generation.wrapping_add(1);
        state.generation
    };

    let _ = app.emit("status", "stopped");

    let mut handles = Vec::new();

    for rule in rules {
        let addrs: Vec<String> = {
            let mut v: Vec<String> = rule
                .listen_addrs
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if v.is_empty() {
                v.push(rule.listen_addr.clone());
            }
            v
        };

        for listen_addr in addrs {
            let app_handle = app.clone();
            let rule_clone = rule.clone();
            let listen_addr_clone = listen_addr.clone();
            let gen = generation;
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

            let handle = tauri::async_runtime::spawn(async move {
                if let Err(e) = precheck_rule(&rule_clone, &listen_addr_clone).await {
                    error!("Failed to start listener({listen_addr_clone}): {e}");
                    send_log(format!(
                        "Failed to start listener({listen_addr_clone}): {e}"
                    ));

                    let payload = super::RuleStartErrorPayload {
                        listen_addr: listen_addr_clone.clone(),
                        error: e.to_string(),
                    };
                    let _ = app_handle.emit("server-start-error", payload);

                    {
                        let mut state = PROXY_STATE.lock();
                        if state.generation == gen {
                            state.phase = Phase::Failed;
                        }
                    }
                    let _ = app_handle.emit("status", "stopped");
                    return;
                }

                let transition_to_running = {
                    let mut state = PROXY_STATE.lock();
                    if state.generation != gen {
                        return;
                    }
                    state.started += 1;
                    if matches!(state.phase, Phase::Starting) && state.started == state.expected {
                        state.phase = Phase::Running;
                        true
                    } else {
                        false
                    }
                };

                if transition_to_running {
                    let _ = app_handle.emit("status", "running");
                }

                match start_rule_server(
                    app_handle.clone(),
                    rule_clone,
                    listen_addr_clone.clone(),
                    shutdown_rx,
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to serve on {listen_addr_clone}: {e}");
                        send_log_with_app(
                            &app_handle,
                            format!("Failed to serve on {listen_addr_clone}: {e}"),
                        );
                    }
                }
            });

            handles.push(ServerHandle {
                handle,
                shutdown_tx,
            });
        }
    }

    {
        let mut state = PROXY_STATE.lock();
        if state.generation == generation {
            state.handles = handles;
        } else {
            for h in handles {
                h.abort();
            }
        }
    }

    info!("Proxy server started");
    Ok(())
}

pub fn stop_server(app: tauri::AppHandle) -> Result<()> {
    ws_proxy::stop_ws_servers();
    *LOG_TX.write() = None;

    tauri::async_runtime::spawn(async {
        stream_proxy::stop_stream_servers().await;
    });

    let handles = {
        let mut state = PROXY_STATE.lock();
        state.phase = Phase::Stopped;
        state.generation = state.generation.wrapping_add(1);
        state.expected = 0;
        state.started = 0;
        std::mem::take(&mut state.handles)
    };

    for handle in handles {
        handle.abort();
    }

    let _ = app.emit("status", "stopped");

    let cfg = config::get_config();
    for r in &cfg.rules {
        let addrs: Vec<String> = {
            let mut v: Vec<String> = r
                .listen_addrs
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if v.is_empty() {
                v.push(r.listen_addr.clone());
            }
            v
        };
        for addr in addrs {
            send_log_with_app(&app, format!("[HTTP NODE {}] Server stopped", addr));
        }
    }

    info!("Proxy server stopped");
    Ok(())
}

pub fn is_running() -> bool {
    matches!(PROXY_STATE.lock().phase, Phase::Running)
}

pub fn is_effectively_running() -> bool {
    matches!(PROXY_STATE.lock().phase, Phase::Starting | Phase::Running)
}
