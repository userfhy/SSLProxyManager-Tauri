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
