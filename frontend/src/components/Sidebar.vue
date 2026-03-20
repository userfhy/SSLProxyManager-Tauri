<template>
  <el-card class="sidebar-nav" :class="{ 'sidebar-collapsed': isCollapsed }" shadow="hover">
    <div class="sidebar-header">
      <el-button @click="toggleSidebar" circle class="collapse-btn" :title="isCollapsed ? $t('sidebar.expand') : $t('sidebar.collapse')">
        <el-icon><Fold v-if="!isCollapsed" /><Expand v-else /></el-icon>
      </el-button>
    </div>
    <el-menu :default-active="activeTab" class="nav-menu" :collapse="isCollapsed" :collapse-transition="true" @select="handleMenuSelect">
      <el-menu-item-group :title="isCollapsed ? '' : $t('sidebar.configManagement')">
        <el-menu-item index="config">
          <el-icon><Connection /></el-icon>
          <template #title>{{ $t('sidebar.proxyConfig') }}</template>
        </el-menu-item>
        <el-menu-item index="ws">
          <el-icon><Promotion /></el-icon>
          <template #title>{{ $t('sidebar.wsProxyConfig') }}</template>
        </el-menu-item>
        <el-menu-item index="stream">
          <el-icon><Share /></el-icon>
          <template #title>{{ $t('sidebar.streamConfig') }}</template>
        </el-menu-item>
        <el-menu-item index="access">
          <el-icon><Lock /></el-icon>
          <template #title>{{ $t('sidebar.accessControl') }}</template>
        </el-menu-item>
        <el-menu-item index="storage">
          <el-icon><Document /></el-icon>
          <template #title>{{ $t('sidebar.dataPersistence') }}</template>
        </el-menu-item>
        <el-menu-item index="base">
          <el-icon><Setting /></el-icon>
          <template #title>{{ $t('sidebar.baseConfig') }}</template>
        </el-menu-item>
      </el-menu-item-group>

      <el-menu-item-group :title="isCollapsed ? '' : $t('sidebar.monitoring')">
        <el-menu-item index="dashboard">
          <el-icon><DataAnalysis /></el-icon>
          <template #title>{{ $t('sidebar.dashboard') }}</template>
        </el-menu-item>
        <el-menu-item index="systemMetrics">
          <el-icon><DataLine /></el-icon>
          <template #title>{{ $t('sidebar.systemMetrics') }}</template>
        </el-menu-item>
      </el-menu-item-group>

      <el-menu-item-group :title="isCollapsed ? '' : $t('sidebar.logs')">
        <el-menu-item index="requestLogs">
          <el-icon><Search /></el-icon>
          <template #title>{{ $t('sidebar.requestLogs') }}</template>
        </el-menu-item>
        <el-menu-item index="logs">
          <el-icon><Document /></el-icon>
          <template #title>{{ $t('sidebar.accessLogs') }}</template>
        </el-menu-item>
      </el-menu-item-group>

      <el-menu-item-group :title="isCollapsed ? '' : $t('sidebar.tools')">
        <el-menu-item index="testTools">
          <el-icon><Tools /></el-icon>
          <template #title>{{ $t('sidebar.testTools') }}</template>
        </el-menu-item>
      </el-menu-item-group>

      <el-menu-item-group :title="isCollapsed ? '' : $t('sidebar.system')">
        <el-menu-item index="about">
          <el-icon><InfoFilled /></el-icon>
          <template #title>{{ $t('sidebar.about') }}</template>
        </el-menu-item>
      </el-menu-item-group>
    </el-menu>
  </el-card>
</template>

<script setup lang="ts">
import { Setting, DataAnalysis, DataLine, Document, Lock, Search, Fold, Expand, InfoFilled, Tools, Connection, Promotion, Share } from '@element-plus/icons-vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

defineProps<{ 
  isCollapsed: boolean;
  activeTab: string;
}>();

const emit = defineEmits(['toggle-sidebar', 'select-menu']);

const toggleSidebar = () => {
  emit('toggle-sidebar');
};

const handleMenuSelect = (key: string) => {
  emit('select-menu', key);
};
</script>

<style scoped>
.sidebar-nav {
  width: 200px;
  flex-shrink: 0;
  border-radius: var(--radius-lg);
  background: var(--card-bg);
  border: 1px solid var(--border);
  backdrop-filter: blur(12px) saturate(180%);
  -webkit-backdrop-filter: blur(12px) saturate(180%);
  height: 100%;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  box-shadow: var(--shadow-sm);
}

.sidebar-nav:hover {
  box-shadow: var(--shadow-md);
}

.sidebar-nav.sidebar-collapsed {
  width: 64px;
  overflow: hidden;
}

.sidebar-nav :deep(.el-card__body) {
  padding: 12px 8px;
  transition: padding 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  height: 100%;
  display: flex;
  flex-direction: column;
}

.sidebar-collapsed :deep(.el-card__body) {
  padding: 12px 4px;
  overflow: hidden;
}

.sidebar-header {
  display: flex;
  justify-content: flex-end;
  padding: 8px 12px 12px;
  border-bottom: 1px solid var(--border);
  margin-bottom: 8px;
}

.sidebar-collapsed .sidebar-header {
  justify-content: center;
  padding: 8px 4px 12px;
}

.collapse-btn {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  background: var(--btn-bg);
  border: 1px solid var(--border);
}

.collapse-btn:hover {
  background: var(--btn-hover);
  box-shadow: var(--shadow-sm);
}

.nav-menu {
  border: none;
  background: transparent;
  flex: 1;
  overflow-y: auto;
}

.nav-menu :deep(.el-menu-item-group__title) {
  padding: 12px 20px 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  transition: all 0.3s ease;
  transition: opacity 0.3s;
}

.sidebar-collapsed .nav-menu :deep(.el-menu-item-group__title) {
  padding: 0;
  height: 0;
  overflow: hidden;
}

.nav-menu :deep(.el-menu-item) {
  height: 44px;
  line-height: 44px;
  margin: 4px 8px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  white-space: nowrap;
  position: relative;
  overflow: hidden;
}

.nav-menu :deep(.el-menu-item)::before {
  content: '';
  position: absolute;
  left: 0;
  top: 0;
  width: 3px;
  height: 100%;
  background: var(--primary);
  transform: scaleY(0);
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.sidebar-collapsed .nav-menu :deep(.el-menu-item) {
  padding: 0 !important;
  justify-content: center;
}

/* Target the specific tooltip trigger that Element Plus uses when collapsed */
.sidebar-collapsed .nav-menu :deep(.el-menu-tooltip__trigger) {
  display: inline-flex;
  justify-content: center;
  align-items: center;
  width: 100%;
  padding: 0;
}

.sidebar-collapsed .nav-menu :deep(.el-menu-item .el-icon) {
  margin: 0;
}

.nav-menu :deep(.el-menu-item:hover) {
  background: var(--primary-light);
  color: var(--primary);
}

.nav-menu :deep(.el-menu-item.is-active) {
  background: var(--primary-light);
  color: var(--primary);
  font-weight: 600;
  box-shadow: 0 2px 8px rgba(79, 156, 249, 0.2);
}

.nav-menu :deep(.el-menu-item.is-active)::before {
  transform: scaleY(1);
}

.nav-menu :deep(.el-menu-item .el-icon) {
  margin-right: 8px;
  font-size: 18px;
  transition: margin 0.3s;
}

.sidebar-collapsed .nav-menu :deep(.el-menu-item .el-icon) {
  margin-right: 0;
}

.nav-menu :deep(.el-menu-item span) {
  transition: opacity 0.3s;
}

.sidebar-collapsed .nav-menu :deep(.el-menu-item span) {
  display: none;
}

/* Element Plus 折叠菜单样式 */
.nav-menu :deep(.el-menu--collapse) {
  width: 100%;
}

.nav-menu :deep(.el-menu--collapse .el-menu-item) {
  padding: 0 20px !important;
}

.sidebar-collapsed .nav-menu {
  overflow-y: auto;
  overflow-x: hidden;
}

.sidebar-collapsed .nav-menu :deep(.el-menu--collapse .el-menu-item) {
  padding: 0 !important;
}

/* 折叠时显示 tooltip */
.sidebar-collapsed .nav-menu :deep(.el-menu-item) {
  position: relative;
}

.sidebar-collapsed .nav-menu :deep(.el-menu-item:hover::after) {
  content: attr(title);
  position: absolute;
  left: 100%;
  top: 50%;
  transform: translateY(-50%);
  margin-left: 12px;
  padding: 6px 12px;
  background: var(--card-bg);
  border: 1px solid var(--border);
  border-radius: 6px;
  white-space: nowrap;
  z-index: 1000;
  font-size: 13px;
  color: var(--text);
  box-shadow: var(--shadow-md);
  pointer-events: none;
}
</style>
