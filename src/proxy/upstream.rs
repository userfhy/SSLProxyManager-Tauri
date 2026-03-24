use dashmap::DashMap;
use std::sync::Arc;

use anyhow::Result;
use axum::http::Uri;
use crate::config;

#[derive(Debug, Clone)]
pub struct SmoothUpstream {
    pub url: String,
    pub weight: i32,
    pub current: i32,
}

#[derive(Debug, Clone)]
pub struct SmoothLbState {
    pub signature: String,
    pub total_weight: i32,
    pub upstreams: Vec<SmoothUpstream>,
}

pub static UPSTREAM_LB: once_cell::sync::Lazy<DashMap<String, Arc<parking_lot::Mutex<SmoothLbState>>>> =
    once_cell::sync::Lazy::new(DashMap::new);

#[inline]
pub fn upstream_signature(route: &config::Route) -> String {
    use std::fmt::Write;
    let mut buf = crate::buffer_pool::acquire_buffer();

    let mut parts: Vec<String> = route
        .upstreams
        .iter()
        .map(|u| format!("{}#{}", u.url, u.weight))
        .collect();
    parts.sort_unstable();

    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            let _ = write!(buf, "|");
        }
        let _ = write!(buf, "{}", part);
    }

    std::str::from_utf8(&buf).unwrap_or("").to_string()
}

#[inline]
pub fn pick_upstream_smooth(route: &config::Route) -> Option<String> {
    match route.upstreams.len() {
        0 => return None,
        1 => return Some(route.upstreams[0].url.clone()),
        _ => {}
    }

    let route_id = route.id.as_deref().unwrap_or("").trim();
    if route_id.is_empty() {
        return Some(route.upstreams[0].url.clone());
    }

    let sig = upstream_signature(route);

    let state_lock = UPSTREAM_LB
        .entry(route_id.to_string())
        .or_insert_with(|| {
            Arc::new(parking_lot::Mutex::new(SmoothLbState {
                signature: String::new(),
                total_weight: 0,
                upstreams: Vec::new(),
            }))
        })
        .clone();

    let mut entry = state_lock.lock();

    if entry.signature != sig || entry.upstreams.len() != route.upstreams.len() {
        let ups: Vec<SmoothUpstream> = route
            .upstreams
            .iter()
            .map(|u| SmoothUpstream {
                url: u.url.clone(),
                weight: std::cmp::max(1, u.weight),
                current: 0,
            })
            .collect();
        let total = ups.iter().map(|u| u.weight).sum::<i32>();

        entry.signature = sig;
        entry.total_weight = std::cmp::max(1, total);
        entry.upstreams = ups;
    }

    let mut best_idx = 0usize;
    for i in 0..entry.upstreams.len() {
        let w = entry.upstreams[i].weight;
        entry.upstreams[i].current = entry.upstreams[i].current.saturating_add(w);
        if entry.upstreams[i].current > entry.upstreams[best_idx].current {
            best_idx = i;
        }
    }

    entry.upstreams[best_idx].current = entry.upstreams[best_idx]
        .current
        .saturating_sub(entry.total_weight);

    Some(entry.upstreams[best_idx].url.clone())
}

pub fn build_upstream_url(
    upstream_base: &str,
    route_path: Option<&str>,
    proxy_pass_path: Option<&str>,
    uri: &Uri,
) -> Result<String> {
    let mut base = upstream_base.trim_end_matches('/').to_string();

    let orig_path = uri.path();
    let route_path = route_path.unwrap_or("/");

    let mut new_path = orig_path.to_string();
    if let Some(pp) = proxy_pass_path {
        let from = if route_path.is_empty() { "/" } else { route_path };
        let to = if pp.trim().is_empty() { "/" } else { pp };

        if new_path.starts_with(from) {
            let suffix = &new_path[from.len()..];

            let mut out_path = to.to_string();
            if out_path.is_empty() {
                out_path = "/".to_string();
            }

            let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
            if out_path.ends_with('/') {
                new_path = if suffix.is_empty() {
                    out_path
                } else {
                    format!("{}{}", out_path, suffix)
                };
            } else {
                new_path = if suffix.is_empty() {
                    out_path
                } else {
                    format!("{}/{}", out_path, suffix)
                };
            }
        }

        if !new_path.starts_with('/') {
            new_path = format!("/{}", new_path);
        }
    }

    base.push_str(&new_path);
    if let Some(q) = uri.query() {
        base.push('?');
        base.push_str(q);
    }
    Ok(base)
}
