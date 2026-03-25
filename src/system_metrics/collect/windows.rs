use super::super::prelude::*;
use super::*;

#[cfg(target_os = "windows")]
#[inline]
fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

#[cfg(target_os = "windows")]
fn utf16z_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&x| x == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len]).trim().to_string()
}

#[cfg(target_os = "windows")]
fn update_windows_load_avg(cpu_usage_percent: f64, timestamp: i64) -> (f64, f64, f64) {
    let mut s = WINDOWS_LOAD_AVG.write();
    if !s.initialized {
        s.initialized = true;
        s.last_ts = timestamp;
        s.load1 = cpu_usage_percent;
        s.load5 = cpu_usage_percent;
        s.load15 = cpu_usage_percent;
        return (s.load1, s.load5, s.load15);
    }

    let dt = (timestamp - s.last_ts).max(1) as f64;
    s.last_ts = timestamp;

    let alpha1 = 1.0 - (-dt / 60.0).exp();
    let alpha5 = 1.0 - (-dt / 300.0).exp();
    let alpha15 = 1.0 - (-dt / 900.0).exp();

    s.load1 += alpha1 * (cpu_usage_percent - s.load1);
    s.load5 += alpha5 * (cpu_usage_percent - s.load5);
    s.load15 += alpha15 * (cpu_usage_percent - s.load15);
    (s.load1, s.load5, s.load15)
}

#[cfg(target_os = "windows")]
fn to_wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn normalize_windows_interface_name(raw: &str) -> Option<String> {
    let mut name = raw.trim().to_string();
    if name.is_empty() {
        return None;
    }

    const FILTER_SUFFIX_MARKERS: [&str; 3] = [
        "-QoS Packet Scheduler-",
        "-WFP 802.3 MAC Layer LightWeight Filter-",
        "-WFP Native MAC Layer LightWeight Filter-",
    ];

    for marker in FILTER_SUFFIX_MARKERS {
        if let Some((base, _)) = name.split_once(marker) {
            name = base.trim().to_string();
            break;
        }
    }

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(target_os = "windows")]
fn pdh_counter_value_double(counter: usize) -> Option<f64> {
    let mut ctype: u32 = 0;
    let mut value: PDH_FMT_COUNTERVALUE = unsafe { zeroed() };
    let status = unsafe {
        PdhGetFormattedCounterValue(
            counter as *mut c_void,
            PDH_FMT_DOUBLE,
            &mut ctype,
            &mut value,
        )
    };
    if status != 0 || value.CStatus != 0 {
        return None;
    }
    Some(unsafe { value.Anonymous.doubleValue }.max(0.0))
}

#[cfg(target_os = "windows")]
fn get_windows_disk_bps() -> (f64, f64) {
    let mut guard = WINDOWS_PDH.write();
    if guard.is_none() {
        let mut query: *mut c_void = null_mut();
        let open_status = unsafe { PdhOpenQueryW(null_mut(), 0, &mut query) };
        if open_status != 0 {
            return (0.0, 0.0);
        }

        let mut read_counter: *mut c_void = null_mut();
        let mut write_counter: *mut c_void = null_mut();
        let read_path = to_wide_z("\\PhysicalDisk(_Total)\\Disk Read Bytes/sec");
        let write_path = to_wide_z("\\PhysicalDisk(_Total)\\Disk Write Bytes/sec");

        let add_read =
            unsafe { PdhAddEnglishCounterW(query, read_path.as_ptr(), 0, &mut read_counter) };
        let add_write =
            unsafe { PdhAddEnglishCounterW(query, write_path.as_ptr(), 0, &mut write_counter) };
        if add_read != 0 || add_write != 0 {
            unsafe { PdhCloseQuery(query) };
            return (0.0, 0.0);
        }

        let _ = unsafe { PdhCollectQueryData(query) };
        *guard = Some(WindowsPdhState {
            query: query as usize,
            read_counter: read_counter as usize,
            write_counter: write_counter as usize,
        });
    }

    let Some(state) = guard.as_ref() else {
        return (0.0, 0.0);
    };

    let status = unsafe { PdhCollectQueryData(state.query as *mut c_void) };
    if status != 0 {
        return (0.0, 0.0);
    }

    let read = pdh_counter_value_double(state.read_counter).unwrap_or(0.0);
    let write = pdh_counter_value_double(state.write_counter).unwrap_or(0.0);
    (read, write)
}

#[cfg(target_os = "windows")]
fn collect_windows_network_stats() -> Result<(u64, u64, Vec<NetworkInterfaceStats>)> {
    let mut table_ptr: *mut MIB_IF_TABLE2 = null_mut();
    let status = unsafe { GetIfTable2(&mut table_ptr) };
    if status != 0 || table_ptr.is_null() {
        return Err(anyhow!("GetIfTable2 failed: status={status}"));
    }

    let mut merged: HashMap<String, (u64, u64)> = HashMap::new();

    unsafe {
        let table = &*table_ptr;
        let rows = std::slice::from_raw_parts(table.Table.as_ptr(), table.NumEntries as usize);
        for row in rows {
            if row.Type == IF_TYPE_SOFTWARE_LOOPBACK as u32 || row.Type == IF_TYPE_TUNNEL as u32 {
                continue;
            }

            let mut name = utf16z_to_string(&row.Alias);
            if name.is_empty() {
                name = utf16z_to_string(&row.Description);
            }
            let Some(name) = normalize_windows_interface_name(&name) else {
                continue;
            };

            let rx = row.InOctets;
            let tx = row.OutOctets;
            if rx == 0 && tx == 0 {
                continue;
            }

            let entry = merged.entry(name).or_insert((0, 0));
            entry.0 = entry.0.max(rx);
            entry.1 = entry.1.max(tx);
        }
        FreeMibTable(table_ptr as *mut c_void);
    }

    let mut total_rx: u64 = 0;
    let mut total_tx: u64 = 0;
    let mut interfaces: Vec<NetworkInterfaceStats> = Vec::with_capacity(merged.len());
    for (name, (rx, tx)) in merged {
        total_rx = total_rx.saturating_add(rx);
        total_tx = total_tx.saturating_add(tx);
        interfaces.push(NetworkInterfaceStats {
            name,
            rx_bytes: to_i64_saturated(rx),
            tx_bytes: to_i64_saturated(tx),
        });
    }

    interfaces.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Ok((total_rx, total_tx, interfaces))
}

#[cfg(target_os = "windows")]
fn collect_windows_tcp_counts() -> (i64, i64, i64) {
    fn collect_v4() -> (i64, i64, i64) {
        let mut size: u32 = 0;
        let mut status = unsafe { GetTcpTable2(null_mut(), &mut size, 0) };
        if status != ERROR_INSUFFICIENT_BUFFER && status != 0 {
            return (0, 0, 0);
        }
        let mut buf = vec![0u8; size as usize];
        status = unsafe { GetTcpTable2(buf.as_mut_ptr() as *mut MIB_TCPTABLE2, &mut size, 0) };
        if status != 0 {
            return (0, 0, 0);
        }
        let table = unsafe { &*(buf.as_ptr() as *const MIB_TCPTABLE2) };
        let rows = unsafe {
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        let mut established = 0i64;
        let mut time_wait = 0i64;
        let mut close_wait = 0i64;
        for row in rows {
            match row.dwState as i32 {
                x if x == MIB_TCP_STATE_ESTAB => established += 1,
                x if x == MIB_TCP_STATE_TIME_WAIT => time_wait += 1,
                x if x == MIB_TCP_STATE_CLOSE_WAIT => close_wait += 1,
                _ => {}
            }
        }
        (established, time_wait, close_wait)
    }

    fn collect_v6() -> (i64, i64, i64) {
        let mut size: u32 = 0;
        let mut status = unsafe { GetTcp6Table2(null_mut(), &mut size, 0) };
        if status != ERROR_INSUFFICIENT_BUFFER && status != 0 {
            return (0, 0, 0);
        }
        let mut buf = vec![0u8; size as usize];
        status = unsafe { GetTcp6Table2(buf.as_mut_ptr() as *mut MIB_TCP6TABLE2, &mut size, 0) };
        if status != 0 {
            return (0, 0, 0);
        }
        let table = unsafe { &*(buf.as_ptr() as *const MIB_TCP6TABLE2) };
        let rows = unsafe {
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        let mut established = 0i64;
        let mut time_wait = 0i64;
        let mut close_wait = 0i64;
        for row in rows {
            match row.State as i32 {
                x if x == MIB_TCP_STATE_ESTAB => established += 1,
                x if x == MIB_TCP_STATE_TIME_WAIT => time_wait += 1,
                x if x == MIB_TCP_STATE_CLOSE_WAIT => close_wait += 1,
                _ => {}
            }
        }
        (established, time_wait, close_wait)
    }

    let (a1, b1, c1) = collect_v4();
    let (a2, b2, c2) = collect_v6();
    (a1 + a2, b1 + b2, c1 + c2)
}

#[cfg(target_os = "windows")]
pub(super) fn collect_raw_snapshot_windows() -> Result<RawSnapshot> {
    let timestamp = chrono::Utc::now().timestamp();

    let mut idle: FILETIME = unsafe { zeroed() };
    let mut kernel: FILETIME = unsafe { zeroed() };
    let mut user: FILETIME = unsafe { zeroed() };
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if ok == 0 {
        return Err(anyhow!("GetSystemTimes failed"));
    }
    let cpu_idle = filetime_to_u64(idle);
    let cpu_total = filetime_to_u64(kernel).saturating_add(filetime_to_u64(user));

    let mut mem: MEMORYSTATUSEX = unsafe { zeroed() };
    mem.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    let mem_ok = unsafe { GlobalMemoryStatusEx(&mut mem) };
    if mem_ok == 0 {
        return Err(anyhow!("GlobalMemoryStatusEx failed"));
    }
    let mem_total_bytes = mem.ullTotalPhys;
    let mem_available_bytes = mem.ullAvailPhys;
    let swap_total_bytes = mem.ullTotalPageFile.saturating_sub(mem.ullTotalPhys);
    let swap_free_bytes = mem.ullAvailPageFile.saturating_sub(mem.ullAvailPhys);

    let (net_rx_bytes, net_tx_bytes, interfaces) =
        collect_windows_network_stats().unwrap_or((0, 0, Vec::new()));

    let (disk_read_bps, disk_write_bps) = get_windows_disk_bps();
    let (disk_read_bytes, disk_write_bytes) = {
        let mut acc = WINDOWS_DISK_ACCUM.write();
        let dt = if acc.2 > 0 {
            (timestamp - acc.2).max(1) as f64
        } else {
            effective_sample_interval_secs() as f64
        };
        acc.0 = acc.0.saturating_add((disk_read_bps * dt) as u64);
        acc.1 = acc.1.saturating_add((disk_write_bps * dt) as u64);
        acc.2 = timestamp;
        (acc.0, acc.1)
    };

    let (tcp_established, tcp_time_wait, tcp_close_wait) = collect_windows_tcp_counts();

    let mut perf: PERFORMANCE_INFORMATION = unsafe { zeroed() };
    let perf_ok =
        unsafe { GetPerformanceInfo(&mut perf, size_of::<PERFORMANCE_INFORMATION>() as u32) };
    let (process_count, fd_used) = if perf_ok != 0 {
        (perf.ProcessCount as i64, perf.HandleCount as u64)
    } else {
        (0, 0)
    };

    let uptime_seconds = unsafe { GetTickCount64() as f64 / 1000.0 };
    let cpu_hint = {
        let prev = LAST_RAW.read();
        if let Some(p) = prev.as_ref() {
            let total_delta = cpu_total.saturating_sub(p.cpu_total);
            let idle_delta = cpu_idle.saturating_sub(p.cpu_idle);
            if total_delta > 0 {
                ((total_delta.saturating_sub(idle_delta)) as f64 / total_delta as f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        }
    };
    let (load1, load5, load15) = update_windows_load_avg(cpu_hint, timestamp);

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
        fd_max: 0,
        procs_running: 0,
        procs_blocked: 0,
        context_switches: 0,
        processes_forked_total: 0,
        uptime_seconds,
    })
}

#[cfg(target_os = "windows")]
pub(super) fn reset_windows_collect_state() {
    if let Some(state) = WINDOWS_PDH.write().take() {
        unsafe { PdhCloseQuery(state.query as *mut c_void) };
    }
    *WINDOWS_DISK_ACCUM.write() = (0, 0, 0);
    *WINDOWS_LOAD_AVG.write() = WindowsLoadAvgState::default();
}
