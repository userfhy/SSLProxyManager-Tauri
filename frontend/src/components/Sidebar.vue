<template>
  <el-card class="sidebar-nav" :class="{ 'sidebar-collapsed': isCollapsed }" shadow="hover">
    <div class="sidebar-header">
      <el-button @click="toggleSidebar" circle class="collapse-btn" :title="isCollapsed ? '展开侧边栏' : '折叠侧边栏'">
        <el-icon><Fold v-if="!isCollapsed" /><Expand v-else /></el-icon>
      </el-button>
    </div>
    <el-menu :default-active="activeTab" class="nav-menu" :collapse="isCollapsed" :collapse-transition="true" @select="handleMenuSelect">
      <el-menu-item-group :title="isCollapsed ? '' : '配置管理'">
        <el-menu-item index="config">
          <el-icon><Setting /></el-icon>
          <template #title>代理配置</template>
        </el-menu-item>
        <el-menu-item index="ws">
          <el-icon><Setting /></el-icon>
          <template #title>WS 代理配置</template>
        </el-menu-item>
        <el-menu-item index="stream">
          <el-icon><Setting /></el-icon>
          <template #title>Stream 配置</template>
        </el-menu-item>
        <el-menu-item index="access">
          <el-icon><Lock /></el-icon>
          <template #title>访问控制</template>
        </el-menu-item>
        <el-menu-item index="storage">
          <el-icon><Document /></el-icon>
          <template #title>数据持久化</template>
        </el-menu-item>
        <el-menu-item index="base">
          <el-icon><Setting /></el-icon>
          <template #title>基础配置</template>
        </el-menu-item>
      </el-menu-item-group>

      <el-menu-item-group :title="isCollapsed ? '' : '监控分析'">
        <el-menu-item index="dashboard">
          <el-icon><DataAnalysis /></el-icon>
          <template #title>仪表盘</template>
        </el-menu-item>
      </el-menu-item-group>

      <el-menu-item-group :title="isCollapsed ? '' : '日志查询'">
        <el-menu-item index="requestLogs">
          <el-icon><Search /></el-icon>
          <template #title>请求记录查询</template>
        </el-menu-item>
        <el-menu-item index="logs">
          <el-icon><Document /></el-icon>
          <template #title>访问日志</template>
        </el-menu-item>
      </el-menu-item-group>

      <el-menu-item-group :title="isCollapsed ? '' : '系统'">
        <el-menu-item index="about">
          <el-icon><InfoFilled /></el-icon>
          <template #title>关于</template>
        </el-menu-item>
      </el-menu-item-group>
    </el-menu>
  </el-card>
</template>

<script setup lang="ts">
import { Setting, DataAnalysis, Document, Lock, Search, Fold, Expand, InfoFilled } from '@element-plus/icons-vue'

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
  backdrop-filter: blur(12px);
  height: 100%;
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.sidebar-nav.sidebar-collapsed {
  width: 64px;
  overflow: hidden; /* 彻底禁止滚动 */
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
  overflow: hidden; /* 彻底禁止滚动 */
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
}

.collapse-btn:hover {
  transform: scale(1.1);
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