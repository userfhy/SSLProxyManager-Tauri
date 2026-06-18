import { computed } from "vue";
import { useI18n } from "vue-i18n";

/**
 * 日期范围快捷选项 composable
 * 用于 el-date-picker 的 type="datetimerange" 模式
 */
export const useDateShortcuts = () => {
  const { t } = useI18n();

  const dateShortcuts = computed(() => [
    {
      text: t("common.dateShortcuts.today"),
      value: () => {
        const now = new Date();
        const start = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 0, 0, 0);
        const end = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 23, 59, 59);
        return [start.getTime(), end.getTime()];
      },
    },
    {
      text: t("common.dateShortcuts.yesterday"),
      value: () => {
        const now = new Date();
        const yesterday = new Date(now);
        yesterday.setDate(yesterday.getDate() - 1);
        const start = new Date(
          yesterday.getFullYear(),
          yesterday.getMonth(),
          yesterday.getDate(),
          0,
          0,
          0,
        );
        const end = new Date(
          yesterday.getFullYear(),
          yesterday.getMonth(),
          yesterday.getDate(),
          23,
          59,
          59,
        );
        return [start.getTime(), end.getTime()];
      },
    },
    {
      text: t("common.dateShortcuts.last7Days"),
      value: () => {
        const now = new Date();
        const start = new Date(now);
        start.setDate(start.getDate() - 6);
        start.setHours(0, 0, 0, 0);
        const end = new Date(now);
        end.setHours(23, 59, 59, 999);
        return [start.getTime(), end.getTime()];
      },
    },
    {
      text: t("common.dateShortcuts.last30Days"),
      value: () => {
        const now = new Date();
        const start = new Date(now);
        start.setDate(start.getDate() - 29);
        start.setHours(0, 0, 0, 0);
        const end = new Date(now);
        end.setHours(23, 59, 59, 999);
        return [start.getTime(), end.getTime()];
      },
    },
    {
      text: t("common.dateShortcuts.thisMonth"),
      value: () => {
        const now = new Date();
        const start = new Date(now.getFullYear(), now.getMonth(), 1, 0, 0, 0);
        const end = new Date(now.getFullYear(), now.getMonth() + 1, 0, 23, 59, 59);
        return [start.getTime(), end.getTime()];
      },
    },
    {
      text: t("common.dateShortcuts.lastMonth"),
      value: () => {
        const now = new Date();
        const start = new Date(now.getFullYear(), now.getMonth() - 1, 1, 0, 0, 0);
        const end = new Date(now.getFullYear(), now.getMonth(), 0, 23, 59, 59);
        return [start.getTime(), end.getTime()];
      },
    },
    {
      text: t("common.dateShortcuts.last6Months"),
      value: () => {
        const now = new Date();
        const start = new Date(now);
        start.setMonth(start.getMonth() - 6);
        start.setDate(1);
        start.setHours(0, 0, 0, 0);
        const end = new Date(now);
        end.setHours(23, 59, 59, 999);
        return [start.getTime(), end.getTime()];
      },
    },
    {
      text: t("common.dateShortcuts.lastYear"),
      value: () => {
        const now = new Date();
        const start = new Date(now.getFullYear() - 1, 0, 1, 0, 0, 0);
        const end = new Date(now.getFullYear() - 1, 11, 31, 23, 59, 59);
        return [start.getTime(), end.getTime()];
      },
    },
    {
      text: t("common.dateShortcuts.thisYear"),
      value: () => {
        const now = new Date();
        const start = new Date(now.getFullYear(), 0, 1, 0, 0, 0);
        const end = new Date(now.getFullYear(), 11, 31, 23, 59, 59);
        return [start.getTime(), end.getTime()];
      },
    },
  ]);

  return { dateShortcuts };
};
