import type { AlertingConfig, AlertWebhookConfig, ConfigSnapshotInfo } from '../../api'

export const DEFAULT_CONNECT_TIMEOUT_MS = 5000
export const DEFAULT_READ_TIMEOUT_MS = 30000
export const DEFAULT_POOL_MAX_IDLE = 100
export const DEFAULT_POOL_IDLE_TIMEOUT_SEC = 60
export const DEFAULT_MAX_BODY_SIZE_MB = 10
export const DEFAULT_MAX_RESPONSE_BODY_SIZE_MB = 10
export const DEFAULT_ENABLE_HTTP2 = true
export const DEFAULT_COMPRESSION_ENABLED = false
export const DEFAULT_COMPRESSION_GZIP = true
export const DEFAULT_COMPRESSION_BROTLI = true
export const DEFAULT_COMPRESSION_MIN_LENGTH = 1024
export const DEFAULT_COMPRESSION_GZIP_LEVEL = 6
export const DEFAULT_COMPRESSION_BROTLI_LEVEL = 6
export const DEFAULT_QUIET_HOURS_START = '23:00'
export const DEFAULT_QUIET_HOURS_END = '08:00'
export const DEFAULT_SYSTEM_REPORT_INTERVAL_MINUTES = 60
export const DEFAULT_SYSTEM_REPORT_WEEKDAYS = [1, 2, 3, 4, 5, 6, 7]
export const SYSTEM_REPORT_INTERVAL_OPTIONS = [5, 10, 15, 30, 60, 120]

export interface AlertWebhookForm extends Omit<AlertWebhookConfig, 'system_report_interval_minutes'> {
  system_report_enabled: boolean
  quiet_hours_enabled: boolean
  quiet_hours_start: string
  quiet_hours_end: string
  system_report_interval_minutes: number | string
  system_report_weekdays: number[]
}

export interface AlertingForm extends Omit<AlertingConfig, 'webhook'> {
  webhook: AlertWebhookForm
}

export interface BaseGeneralForm {
  autoStart: boolean
  showRealtimeLogs: boolean
  realtimeLogsOnlyErrors: boolean
  streamProxy: boolean
  enableHttp2: boolean
  maxBodySizeMB: number
  maxResponseBodySizeMB: number
  upstreamConnectTimeoutMs: number
  upstreamReadTimeoutMs: number
  upstreamPoolMaxIdle: number
  upstreamPoolIdleTimeoutSec: number
  compressionEnabled: boolean
  compressionGzip: boolean
  compressionBrotli: boolean
  compressionGzipLevel: number
  compressionBrotliLevel: number
}

export interface BaseSnapshotsForm {
  loading: boolean
  restoringSnapshotName: string
  list: ConfigSnapshotInfo[]
}
