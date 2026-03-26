<template>
  <el-form label-width="180px">
    <el-form-item :label="$t('about.configSnapshots')">
      <el-button @click="$emit('refresh')" :loading="model.loading">{{ $t('about.refreshSnapshots') }}</el-button>
    </el-form-item>

    <el-form-item>
      <el-table :data="model.list" style="width: 100%" size="small" v-loading="model.loading">
        <el-table-column prop="name" :label="$t('about.snapshotName')" min-width="240" />
        <el-table-column :label="$t('about.snapshotTime')" min-width="180">
          <template #default="scope">
            {{ formatTs(scope.row.created_at_unix_ms) }}
          </template>
        </el-table-column>
        <el-table-column :label="$t('about.snapshotSize')" width="120">
          <template #default="scope">
            {{ formatSize(scope.row.size_bytes) }}
          </template>
        </el-table-column>
        <el-table-column :label="$t('about.actions')" width="120">
          <template #default="scope">
            <el-button
              size="small"
              type="warning"
              class="snapshot-restore-btn"
              @click="$emit('restore', scope.row.name)"
              :loading="model.restoringSnapshotName === scope.row.name"
            >
              {{ $t('about.restoreSnapshot') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-form-item>
  </el-form>
</template>

<script setup lang="ts">
import type { BaseSnapshotsForm } from './types'

defineProps<{
  model: BaseSnapshotsForm
  formatTs: (ms: number) => string
  formatSize: (size: number) => string
}>()

defineEmits<{
  (e: 'refresh'): void
  (e: 'restore', name: string): void
}>()
</script>

<style scoped>
.light-mode .snapshot-restore-btn {
  --el-button-bg-color: #f3bf60;
  --el-button-border-color: #d18a16;
  --el-button-text-color: #ffffff;
  --el-button-hover-bg-color: #ecb149;
  --el-button-hover-border-color: #b97511;
  --el-button-hover-text-color: #ffffff;
  --el-button-active-bg-color: #e4a63b;
  --el-button-active-border-color: #9d620f;
  --el-button-active-text-color: #ffffff;
  color: #ffffff !important;
  border-color: #d18a16 !important;
  background: linear-gradient(135deg, #f3bf60, #e7a531) !important;
  box-shadow: 0 4px 12px rgba(209, 138, 22, 0.2) !important;
}

.light-mode .snapshot-restore-btn:hover:not(:disabled),
.light-mode .snapshot-restore-btn:focus:not(:disabled),
.light-mode .snapshot-restore-btn:active:not(:disabled) {
  color: #ffffff !important;
  border-color: #9d620f !important;
  box-shadow: 0 8px 20px rgba(209, 138, 22, 0.26), 0 0 0 2px rgba(209, 138, 22, 0.16) !important;
}
</style>
