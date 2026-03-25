use super::*;

#[cfg(target_os = "linux")]
fn parse_proc_stat() -> Result<(u64, u64, i64, i64, u64, u64)> {
    let content =
        std::fs::read_to_string("/proc/stat").with_context(|| "failed to read /proc/stat")?;

    let mut cpu_total = 0u64;
    let mut cpu_idle = 0u64;
    let mut procs_running = 0i64;
    let mut procs_blocked = 0i64;
    let mut context_switches = 0u64;
    let mut processes_forked_total = 0u64;

    for line in content.lines() {
        if line.starts_with("cpu ") {
            let mut total = 0u64;
            let mut idle = 0u64;
            let mut iowait = 0u64;
            let mut count = 0usize;

            for (idx, v) in line
                .split_whitespace()
                .skip(1)
                .filter_map(|s| s.parse::<u64>().ok())
                .enumerate()
            {
                total = total.saturating_add(v);
                if idx == 3 {
                    idle = v;
                } else if idx == 4 {
                    iowait = v;
                }
                count += 1;
            }

            if count >= 4 {
                cpu_total = total;
                cpu_idle = idle.saturating_add(iowait);
            }
        } else if let Some(v) = line.strip_prefix("procs_running ") {
            procs_running = v.trim().parse::<i64>().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("procs_blocked ") {
            procs_blocked = v.trim().parse::<i64>().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("ctxt ") {
            context_switches = v.trim().parse::<u64>().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("processes ") {
            processes_forked_total = v.trim().parse::<u64>().unwrap_or(0);
        }
    }

    Ok((
        cpu_total,
        cpu_idle,
        procs_running,
        procs_blocked,
        context_switches,
        processes_forked_total,
    ))
}

#[cfg(target_os = "linux")]
fn parse_proc_meminfo() -> Result<(u64, u64, u64, u64)> {
    let content =
        std::fs::read_to_string("/proc/meminfo").with_context(|| "failed to read /proc/meminfo")?;

    let mut mem_total_kb = 0u64;
    let mut mem_available_kb = 0u64;
    let mut swap_total_kb = 0u64;
    let mut swap_free_kb = 0u64;

    for line in content.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            mem_total_kb = v
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("MemAvailable:") {
            mem_available_kb = v
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("SwapTotal:") {
            swap_total_kb = v
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("SwapFree:") {
            swap_free_kb = v
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(0);
        }
    }

    Ok((
        mem_total_kb.saturating_mul(1024),
        mem_available_kb.saturating_mul(1024),
        swap_total_kb.saturating_mul(1024),
        swap_free_kb.saturating_mul(1024),
    ))
}

#[cfg(target_os = "linux")]
fn parse_proc_loadavg() -> Result<(f64, f64, f64)> {
    let content =
        std::fs::read_to_string("/proc/loadavg").with_context(|| "failed to read /proc/loadavg")?;

    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.len() < 3 {
        return Err(anyhow!("invalid /proc/loadavg format"));
    }

    let l1 = parts[0].parse::<f64>().unwrap_or(0.0);
    let l5 = parts[1].parse::<f64>().unwrap_or(0.0);
    let l15 = parts[2].parse::<f64>().unwrap_or(0.0);
    Ok((l1, l5, l15))
}

#[cfg(target_os = "linux")]
fn parse_proc_uptime() -> Result<f64> {
    let content =
        std::fs::read_to_string("/proc/uptime").with_context(|| "failed to read /proc/uptime")?;
    let first = content
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("invalid /proc/uptime format"))?;
    Ok(first.parse::<f64>().unwrap_or(0.0))
}

#[cfg(target_os = "linux")]
fn parse_proc_net_dev() -> Result<(u64, u64, Vec<NetworkInterfaceStats>)> {
    let content =
        std::fs::read_to_string("/proc/net/dev").with_context(|| "failed to read /proc/net/dev")?;

    let mut total_rx: u64 = 0;
    let mut total_tx: u64 = 0;
    let mut interfaces: Vec<NetworkInterfaceStats> = Vec::new();

    for line in content.lines().skip(2) {
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }

        let Some((iface_name, counters)) = raw.split_once(':') else {
            continue;
        };

        let name = iface_name.trim();
        if name.is_empty() || name == "lo" {
            continue;
        }

        let cols: Vec<&str> = counters.split_whitespace().collect();
        if cols.len() < 16 {
            continue;
        }

        let rx = cols[0].parse::<u64>().unwrap_or(0);
        let tx = cols[8].parse::<u64>().unwrap_or(0);

        total_rx = total_rx.saturating_add(rx);
        total_tx = total_tx.saturating_add(tx);

        interfaces.push(NetworkInterfaceStats {
            name: name.to_string(),
            rx_bytes: to_i64_saturated(rx),
            tx_bytes: to_i64_saturated(tx),
        });
    }

    interfaces.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    Ok((total_rx, total_tx, interfaces))
}

#[cfg(target_os = "linux")]
fn parse_proc_diskstats() -> Result<(u64, u64)> {
    let content = std::fs::read_to_string("/proc/diskstats")
        .with_context(|| "failed to read /proc/diskstats")?;

    let mut read_sectors: u64 = 0;
    let mut write_sectors: u64 = 0;

    for line in content.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 14 {
            continue;
        }

        let name = cols[2];
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("fd")
            || name.starts_with("sr")
        {
            continue;
        }

        let r = cols[5].parse::<u64>().unwrap_or(0);
        let w = cols[9].parse::<u64>().unwrap_or(0);
        read_sectors = read_sectors.saturating_add(r);
        write_sectors = write_sectors.saturating_add(w);
    }

    Ok((
        read_sectors.saturating_mul(512),
        write_sectors.saturating_mul(512),
    ))
}

#[cfg(target_os = "linux")]
fn parse_tcp_states_from(path: &str) -> (i64, i64, i64) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return (0, 0, 0);
    };

    let mut established = 0i64;
    let mut time_wait = 0i64;
    let mut close_wait = 0i64;

    for line in content.lines().skip(1) {
        let mut cols = line.split_whitespace();
        let _ = cols.next();
        let _ = cols.next();
        let _ = cols.next();
        let Some(state) = cols.next() else {
            continue;
        };

        match state {
            "01" => established += 1,
            "06" => time_wait += 1,
            "08" => close_wait += 1,
            _ => {}
        }
    }

    (established, time_wait, close_wait)
}

#[cfg(target_os = "linux")]
fn parse_tcp_states() -> (i64, i64, i64) {
    let (a1, b1, c1) = parse_tcp_states_from("/proc/net/tcp");
    let (a2, b2, c2) = parse_tcp_states_from("/proc/net/tcp6");
    (a1 + a2, b1 + b2, c1 + c2)
}

#[cfg(target_os = "linux")]
fn count_processes() -> i64 {
    let Ok(iter) = std::fs::read_dir("/proc") else {
        return 0;
    };

    let mut count = 0i64;
    for entry in iter.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
                count += 1;
            }
        }
    }
    count
}

#[cfg(target_os = "linux")]
fn parse_file_nr() -> (u64, u64) {
    let Ok(content) = std::fs::read_to_string("/proc/sys/fs/file-nr") else {
        return (0, 0);
    };

    let mut nums = content
        .split_whitespace()
        .filter_map(|v| v.parse::<u64>().ok());

    let allocated = nums.next();
    let unused = nums.next();
    let max = nums.next();

    if let (Some(allocated), Some(unused), Some(max)) = (allocated, unused, max) {
        (allocated.saturating_sub(unused), max)
    } else {
        (0, 0)
    }
}

#[cfg(target_os = "linux")]
pub(super) fn collect_raw_snapshot() -> Result<RawSnapshot> {
    let timestamp = chrono::Utc::now().timestamp();

    let (
        cpu_total,
        cpu_idle,
        procs_running,
        procs_blocked,
        context_switches,
        processes_forked_total,
    ) = parse_proc_stat()?;
    let (load1, load5, load15) = parse_proc_loadavg()?;
    let (mem_total_bytes, mem_available_bytes, swap_total_bytes, swap_free_bytes) =
        parse_proc_meminfo()?;
    let (net_rx_bytes, net_tx_bytes, interfaces) = parse_proc_net_dev()?;
    let (disk_read_bytes, disk_write_bytes) = parse_proc_diskstats()?;
    let (tcp_established, tcp_time_wait, tcp_close_wait) = parse_tcp_states();
    let process_count = count_processes();
    let (fd_used, fd_max) = parse_file_nr();
    let uptime_seconds = parse_proc_uptime()?;

    Ok(RawSnapshot {
        timestamp,
        cpu_total,
        cpu_idle,
        cpu_usage_percent_hint: None,
        load1,
        load5,
        load15,
        mem_total_bytes,
        mem_available_bytes,
        swap_total_bytes,
        swap_free_bytes,
        net_rx_bytes,
        net_tx_bytes,
        interfaces,
        disk_read_bytes,
        disk_write_bytes,
        tcp_established,
        tcp_time_wait,
        tcp_close_wait,
        process_count,
        fd_used,
        fd_max,
        procs_running,
        procs_blocked,
        context_switches,
        processes_forked_total,
        uptime_seconds,
    })
}
