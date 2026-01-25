// Tauri API 适配层
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-shell';
import { exit, relaunch } from '@tauri-apps/plugin-process';

// 配置相关
export async function GetConfig() {
  return await invoke('get_config');
}

export async function SaveConfig(config: any) {
  return await invoke('save_config', { cfg: config });
}

// 版本相关
export async function GetVersion() {
  return await invoke('get_version');
}

// 更新检查
export async function CheckUpdate() {
  return await invoke('check_update');
}

// 打开 URL
export async function OpenURL(url: string) {
  await open(url);
}

// 服务器控制
export async function StartServer() {
  return await invoke('start_server');
}

export async function StopServer() {
  return await invoke('stop_server');
}

export async function GetStatus() {
  return await invoke('get_status');
}

export async function SetTrayProxyState(running: boolean) {
  return await invoke('set_tray_proxy_state', { running });
}

// 日志相关
export async function GetLogs() {
  return await invoke('get_logs');
}

export async function ClearLogs() {
  return await invoke('clear_logs');
}

// 指标相关
export async function GetMetrics() {
  return await invoke('get_metrics');
}

export async function GetListenAddrs() {
  return await invoke('get_listen_addrs');
}

export async function QueryHistoricalMetrics(req: any) {
  return await invoke('query_historical_metrics', { req });
}

export async function QueryRequestLogs(req: any) {
  return await invoke('query_request_logs', { req });
}

export async function GetDashboardStats(req: any) {
  return await invoke('get_dashboard_stats', { req });
}

// 黑名单相关
export async function AddBlacklistEntry(ip: string, reason: string, durationSeconds: number) {
  return await invoke('add_blacklist_entry', { ip, reason, durationSeconds });
}

export async function RemoveBlacklistEntry(ip: string) {
  return await invoke('remove_blacklist_entry', { ip });
}

export async function GetBlacklistEntries() {
  return await invoke('get_blacklist_entries');
}

export async function RefreshBlacklistCache() {
  return await invoke('refresh_blacklist_cache');
}

// 数据库相关
export async function GetMetricsDBStatus() {
  return await invoke('get_metrics_db_status');
}

export async function GetMetricsDBStatusDetail() {
  return await invoke('get_metrics_db_status_detail');
}

export async function TestMetricsDBConnection(dbPath: string) {
  return await invoke('test_metrics_db_connection', { dbPath });
}

// 文件对话框
export async function OpenCertFileDialog() {
  return await invoke('open_cert_file_dialog');
}

export async function OpenKeyFileDialog() {
  return await invoke('open_key_file_dialog');
}

export async function OpenDirectoryDialog() {
  return await invoke('open_directory_dialog');
}

export async function SaveConfigTomlAs(content: string) {
  return await invoke('save_config_toml_as', { content });
}

export async function ExportCurrentConfigToml() {
  return await invoke('export_current_config_toml');
}

// 规则/路由启用开关
export async function SetListenRuleEnabled(listenRuleId: string, enabled: boolean) {
  return await invoke('set_listen_rule_enabled', { args: { listenRuleId, enabled } });
}

export async function SetRouteEnabled(listenRuleId: string, routeId: string, enabled: boolean) {
  return await invoke('set_route_enabled', { args: { listenRuleId, routeId, enabled } });
}

// 应用控制
export async function HideToTray() {
  return await invoke('hide_to_tray');
}

export async function QuitApp() {
  return await invoke('quit_app');
}

// 条款接受状态（使用 localStorage）
const TERMS_ACCEPTED_KEY = 'ssl_proxy_manager_terms_accepted'

export function GetTermsAccepted(): boolean {
  try {
    const value = localStorage.getItem(TERMS_ACCEPTED_KEY)
    return value === 'true'
  } catch (e) {
    console.error('读取条款接受状态失败:', e)
    return false
  }
}

export async function SetTermsAccepted(accepted: boolean): Promise<void> {
  try {
    if (accepted) {
      localStorage.setItem(TERMS_ACCEPTED_KEY, 'true')
      await relaunch();
    } else {
      localStorage.removeItem(TERMS_ACCEPTED_KEY)
    }
  } catch (e) {
    console.error('保存条款接受状态失败:', e)
    throw e
  }
}

export async function ResetTermsAccepted(): Promise<void> {
  try {
    localStorage.removeItem(TERMS_ACCEPTED_KEY)
    await relaunch();
  } catch (e) {
    console.error('重置条款接受状态失败:', e)
    throw e
  }
}

// 事件监听
export async function EventsOn(event: string, callback: (data: any) => void) {
  const unlisten = await listen(event, (event) => {
    callback(event.payload);
  });
  // 返回取消监听的函数
  return unlisten;
}

// 事件取消监听（Tauri 中通过返回的 unlisten 函数实现）
export function EventsOff(unlisten: (() => void) | null) {
  if (unlisten) {
    unlisten();
  }
}
