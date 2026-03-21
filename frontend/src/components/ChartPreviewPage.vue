<template>
  <div class="page">
    <div class="toolbar">
      <h1 id="title" class="title">Chart Preview</h1>
      <div class="tools">
        <label class="auto-group" for="auto-refresh-toggle">
          <input id="auto-refresh-toggle" type="checkbox" />
          <span id="auto-refresh-label">Auto Refresh</span>
        </label>
        <label class="auto-group" for="auto-refresh-seconds">
          <input id="auto-refresh-seconds" type="number" min="1" max="3600" step="1" value="10" />
          <span id="auto-refresh-seconds-label">sec</span>
        </label>
        <span id="refresh-status" class="status"></span>
        <button id="refresh-btn" class="refresh-btn" type="button">Refresh</button>
        <button id="export-png-btn" class="refresh-btn" type="button">Export PNG</button>
      </div>
    </div>
    <div class="chart-wrap">
      <div id="chart" class="empty">Loading preview...</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'

onMounted(async () => {
  await import('../chart-preview-main')
})
</script>

<style scoped>
.page {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  padding: 14px;
  gap: 10px;
  background: var(--bg-gradient);
  color: var(--text);
}

.title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--card-bg) 88%, transparent);
  backdrop-filter: blur(10px);
}

.tools {
  display: flex;
  align-items: center;
  gap: 8px;
}

.auto-group {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-muted);
}

.auto-group input[type="number"] {
  width: 64px;
  height: 28px;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 0 8px;
  outline: none;
  color: var(--text);
  background: var(--input-bg);
}

.refresh-btn {
  border: 1px solid var(--border);
  background: var(--btn-bg);
  color: var(--text);
  border-radius: 8px;
  height: 30px;
  padding: 0 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.refresh-btn:hover {
  background: var(--btn-hover);
  border-color: var(--border-hover);
}

.refresh-btn:disabled {
  cursor: not-allowed;
  color: var(--text-muted);
  background: var(--btn-bg);
  opacity: 0.7;
}

.status {
  font-size: 12px;
  color: var(--text-muted);
}

.chart-wrap {
  flex: 1;
  min-height: 0;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--card-bg);
  overflow: hidden;
  box-shadow: var(--shadow-sm);
}

#chart {
  width: 100%;
  height: 100%;
}

.empty {
  display: grid;
  place-items: center;
  height: 100%;
  color: var(--text-muted);
  font-size: 14px;
}
</style>
