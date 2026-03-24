// Tauri API 适配层
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import { relaunch } from '@tauri-apps/plugin-process';

export type AppStatus = 'running' | 'stopped'

export interface MetricsDBStatus {
  enabled?: boolean
  connected?: boolean
  dbPath?: string
  error?: string | null
  [key: string]: unknown
}

export interface SaveDialogResult extends String {}

export interface QueryMetricsRequest {
  start_time: number
  end_time: number
  listen_addr?: string | null
}

export interface QueryRequestLogsRequest {
  start_time: number
  end_time: number
  listen_addr?: string | null
  upstream?: string | null
  request_path?: string | null
  client_ip?: string | null
  status_code?: number | null
  page: number
  page_size: number
  matched_route_id?: string | null
  sort_by?: string | null
  sort_order?: string | null
}

export interface DashboardStatsRequest {
  start_time: number
  end_time: number
  listen_addr?: string | null
  granularity_secs: number
}

export interface EventUnlisten {
  (): void
}

export async function GetConfig<T = unknown>(): Promise<T> {
  return await invoke<T>('get_config');
}

export async function SaveConfig<TConfig = unknown, TResult = TConfig>(config: TConfig): Promise<TResult> {
  return await invoke<TResult>('save_config', { cfg: config });
}

export async function GetVersion(): Promise<string> {
  return await invoke<string>('get_version');
}

export async function CheckUpdate<T = unknown>(): Promise<T> {
  return await invoke<T>('check_update');
}

export async function OpenURL(url: string): Promise<void> {
  await openUrl(url);
}

export async function StartServer(): Promise<void> {
  return await invoke<void>('start_server');
}

export async function StopServer(): Promise<void> {
  return await invoke<void>('stop_server');
}

export async function GetStatus(): Promise<AppStatus> {
  return await invoke<AppStatus>('get_status');
}

export async function SetTrayProxyState(running: boolean): Promise<void> {
  return await invoke<void>('set_tray_proxy_state', { running });
}

export async function GetLogs(): Promise<string[]> {
  return await invoke<string[]>('get_logs');
}

export async function ClearLogs(): Promise<void> {
  return await invoke<void>('clear_logs');
}

export async function GetMetrics<T = unknown>(): Promise<T> {
  return await invoke<T>('get_metrics');
}

export async function GetSystemMetrics<T = unknown>(windowSeconds?: number): Promise<T> {
  return await invoke<T>('get_system_metrics', { windowSeconds });
}

export async function SetSystemMetricsSubscription(active: boolean): Promise<void> {
  return await invoke<void>('set_system_metrics_subscription', { active });
}

export async function QueryHistoricalSystemMetrics<T = unknown>(req: T): Promise<T> {
  return await invoke<T>('query_historical_system_metrics', { req });
}

export async function GetListenAddrs(): Promise<string[]> {
  return await invoke<string[]>('get_listen_addrs');
}

export async function QueryHistoricalMetrics<T = unknown>(req: QueryMetricsRequest): Promise<T> {
  return await invoke<T>('query_historical_metrics', { req });
}

export async function QueryRequestLogs<T = unknown>(req: QueryRequestLogsRequest): Promise<T> {
  return await invoke<T>('query_request_logs', { req });
}

export async function GetDashboardStats<T = unknown>(req: DashboardStatsRequest): Promise<T> {
  return await invoke<T>('get_dashboard_stats', { req });
}

export async function AddBlacklistEntry<T = unknown>(ip: string, reason: string, durationSeconds: number): Promise<T> {
  return await invoke<T>('add_blacklist_entry', { ip, reason, durationSeconds });
}

export async function RemoveBlacklistEntry(ip: string): Promise<void> {
  return await invoke<void>('remove_blacklist_entry', { ip });
}

export async function GetBlacklistEntries<T = unknown>(): Promise<T> {
  return await invoke<T>('get_blacklist_entries');
}

export async function RefreshBlacklistCache(): Promise<void> {
  return await invoke<void>('refresh_blacklist_cache');
}

export async function GetMetricsDBStatus(): Promise<MetricsDBStatus> {
  return await invoke<MetricsDBStatus>('get_metrics_db_status');
}

export async function GetMetricsDBStatusDetail(): Promise<MetricsDBStatus> {
  return await invoke<MetricsDBStatus>('get_metrics_db_status_detail');
}

export async function TestMetricsDBConnection(dbPath: string): Promise<[boolean, string]> {
  return await invoke<[boolean, string]>('test_metrics_db_connection', { dbPath });
}

export async function OpenCertFileDialog(): Promise<string | null> {
  return await invoke<string | null>('open_cert_file_dialog');
}

export async function OpenKeyFileDialog(): Promise<string | null> {
  return await invoke<string | null>('open_key_file_dialog');
}

export async function OpenDirectoryDialog(): Promise<string | null> {
  return await invoke<string | null>('open_directory_dialog');
}

export async function OpenDbFileDialog(): Promise<string | null> {
  return await invoke<string | null>('open_db_file_dialog');
}

export async function OpenExistingDbFileDialog(): Promise<string | null> {
  return await invoke<string | null>('open_existing_db_file_dialog');
}

export async function SaveConfigTomlAs(content: string): Promise<string | null> {
  return await invoke<string | null>('save_config_toml_as', { content });
}

export async function ExportCurrentConfigToml(): Promise<string | null> {
  return await invoke<string | null>('export_current_config_toml');
}

export async function SaveChartPngWithDialog(defaultFileName: string, pngDataUrl: string): Promise<string | null> {
  return await invoke<string | null>('save_chart_png_with_dialog', { defaultFileName, pngDataUrl });
}

export async function SetListenRuleEnabled<T = unknown>(listenRuleId: string, enabled: boolean): Promise<T> {
  return await invoke<T>('set_listen_rule_enabled', { args: { listenRuleId, enabled } });
}

export async function SetRouteEnabled<T = unknown>(listenRuleId: string, routeId: string, enabled: boolean): Promise<T> {
  return await invoke<T>('set_route_enabled', { args: { listenRuleId, routeId, enabled } });
}

export async function HideToTray(): Promise<void> {
  return await invoke<void>('hide_to_tray');
}

export async function QuitApp(): Promise<void> {
  return await invoke<void>('quit_app');
}

export async function OpenChartPreviewWindow(title: string, payloadKey: string, windowKey?: string): Promise<void> {
  return await invoke<void>('open_chart_preview_window', { title, payloadKey, windowKey });
}

export async function SetLocale(locale: string): Promise<void> {
  return await invoke<void>('set_locale', { locale });
}

export async function GetLocale(): Promise<string> {
  return await invoke<string>('get_locale');
}

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

export async function EventsOn<T = unknown>(event: string, callback: (data: T) => void): Promise<EventUnlisten> {
  const unlisten = await listen(event, (event) => {
    callback(event.payload as T);
  });
  return unlisten;
}

export function EventsOff(unlisten: EventUnlisten | null) {
  if (unlisten) {
    unlisten();
  }
}
