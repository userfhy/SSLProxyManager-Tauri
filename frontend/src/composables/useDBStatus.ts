import { ref, Ref } from "vue";
import { ElMessage } from "element-plus";
// @ts-ignore
import { GetMetricsDBStatusDetail } from "../api";
import { useI18n } from "vue-i18n";

export interface DBStatus {
  enabled: boolean;
  initialized: boolean;
  path: string;
  error?: string;
  file_exists: boolean;
  dir_exists: boolean;
  dir_writable: boolean;
  message?: string;
  request_logs_count?: number;
  request_logs_min_ts?: number;
  request_logs_max_ts?: number;
  db_file_size_bytes?: number;
  sqlite_version?: string;
  journal_mode?: string;
  synchronous?: string;
  wal_autocheckpoint?: number;
  page_size?: number;
  page_count?: number;
  freelist_count?: number;
  cache_size?: number;
  busy_timeout_ms?: number;
  wal_file_size_bytes?: number;
  shm_file_size_bytes?: number;
}

// 全局数据库状态（所有组件共享同一份数据，但每个组件有独立的加载状态）
const sharedDBStatus = ref<DBStatus | null>(null);

/**
 * 数据库状态管理的 composable
 * 提供统一的数据库状态检查和共享状态管理
 *
 * @returns { dbStatus, loading, checkDBStatus }
 * - dbStatus: 共享的数据库状态（响应式）
 * - loading: 当前组件的加载状态（独立）
 * - checkDBStatus: 检查数据库状态的方法
 */
export const useDBStatus = () => {
  const { t } = useI18n();
  // 每个组件实例有独立的加载状态
  const loading = ref(false);

  /**
   * 检查数据库状态
   * @param showMessage 是否显示成功/错误消息
   * @returns 数据库状态对象
   */
  const checkDBStatus = async (showMessage = false): Promise<DBStatus | null> => {
    loading.value = true;
    try {
      const status = (await GetMetricsDBStatusDetail()) as unknown as DBStatus;
      // 更新共享状态
      sharedDBStatus.value = status;

      if (showMessage) {
        if (status.initialized && status.file_exists) {
          ElMessage.success(t("metricsStorage.databaseStatusNormal"));
        } else if (status.error) {
          ElMessage.warning(t("metricsStorage.databaseStatusAbnormal", { error: status.error }));
        }
      }

      return status;
    } catch (error: any) {
      console.error("获取数据库状态失败:", error);
      const errorMsg = error?.message || String(error);

      if (showMessage) {
        ElMessage.error(t("metricsStorage.databaseStatusAbnormal", { error: errorMsg }));
      }

      return null;
    } finally {
      loading.value = false;
    }
  };

  return {
    dbStatus: sharedDBStatus as Ref<DBStatus | null>, // 返回共享状态
    loading, // 返回独立的加载状态
    checkDBStatus,
  };
};
