<template>
  <el-form label-width="190px">
    <el-form-item :label="$t('baseConfig.autoStart')">
      <el-switch v-model="model.autoStart" />
      <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
        {{ $t('baseConfig.autoStartHint') }}
      </el-text>
    </el-form-item>

    <el-form-item :label="$t('baseConfig.showRealtimeLogs')">
      <el-switch v-model="model.showRealtimeLogs" />
      <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
        {{ $t('baseConfig.showRealtimeLogsHint') }}
      </el-text>
    </el-form-item>

    <el-form-item v-if="model.showRealtimeLogs" :label="$t('baseConfig.realtimeLogsOnlyErrors')">
      <el-switch v-model="model.realtimeLogsOnlyErrors" />
      <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
        {{ $t('baseConfig.realtimeLogsOnlyErrorsHint') }}
      </el-text>
    </el-form-item>

    <el-form-item :label="$t('baseConfig.streamProxy')">
      <el-switch v-model="model.streamProxy" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
      <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
        {{ $t('baseConfig.streamProxyHint') }}
      </el-text>
    </el-form-item>

    <el-form-item v-if="!model.streamProxy" :label="$t('baseConfig.maxBodySizeMB')">
      <el-input-number v-model="model.maxBodySizeMB" :min="1" :max="1024" :step="1" controls-position="right" />
      <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
        {{ $t('baseConfig.maxBodySizeMBHint') }}
      </el-text>
    </el-form-item>

    <el-form-item v-if="!model.streamProxy" :label="$t('baseConfig.maxResponseBodySizeMB')">
      <el-input-number v-model="model.maxResponseBodySizeMB" :min="1" :max="1024" :step="1" controls-position="right" />
      <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
        {{ $t('baseConfig.maxResponseBodySizeMBHint') }}
      </el-text>
    </el-form-item>

    <el-form-item :label="$t('baseConfig.upstreamConnectTimeoutMs')">
      <el-input-number v-model="model.upstreamConnectTimeoutMs" :min="100" :max="600000" :step="100" controls-position="right" />
    </el-form-item>

    <el-form-item :label="$t('baseConfig.upstreamReadTimeoutMs')">
      <el-input-number v-model="model.upstreamReadTimeoutMs" :min="100" :max="600000" :step="100" controls-position="right" />
    </el-form-item>

    <el-form-item :label="$t('baseConfig.upstreamPoolMaxIdle')">
      <el-input-number v-model="model.upstreamPoolMaxIdle" :min="0" :max="1024" :step="1" controls-position="right" />
    </el-form-item>

    <el-form-item :label="$t('baseConfig.enableHttp2')">
      <el-switch v-model="model.enableHttp2" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
      <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
        {{ $t('baseConfig.enableHttp2Hint') }}
      </el-text>
    </el-form-item>

    <el-form-item :label="$t('baseConfig.upstreamPoolIdleTimeoutSec')">
      <el-input-number v-model="model.upstreamPoolIdleTimeoutSec" :min="0" :max="3600" :step="1" controls-position="right" />
    </el-form-item>

    <el-divider />

    <el-form-item :label="$t('baseConfig.compressionEnabled')">
      <el-switch v-model="model.compressionEnabled" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
      <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
        {{ $t('baseConfig.compressionEnabledHint') }}
      </el-text>
    </el-form-item>

    <template v-if="model.compressionEnabled">
      <el-form-item :label="$t('baseConfig.compressionGzip')">
        <el-switch v-model="model.compressionGzip" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          {{ $t('baseConfig.compressionGzipHint') }}
        </el-text>
      </el-form-item>

      <el-form-item v-if="model.compressionGzip" :label="$t('baseConfig.compressionGzipLevel')">
        <el-slider
          v-model="model.compressionGzipLevel"
          :min="1"
          :max="9"
          :step="1"
          show-stops
          show-input
          :show-input-controls="false"
          style="width: 300px; margin-right: 12px;"
        />
        <el-text type="info" size="small" class="mini-hint">
          {{ $t('baseConfig.compressionGzipLevelHint') }}
        </el-text>
      </el-form-item>

      <el-form-item :label="$t('baseConfig.compressionBrotli')">
        <el-switch v-model="model.compressionBrotli" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          {{ $t('baseConfig.compressionBrotliHint') }}
        </el-text>
      </el-form-item>

      <el-form-item v-if="model.compressionBrotli" :label="$t('baseConfig.compressionBrotliLevel')">
        <el-slider
          v-model="model.compressionBrotliLevel"
          :min="0"
          :max="11"
          :step="1"
          show-stops
          show-input
          :show-input-controls="false"
          style="width: 300px; margin-right: 12px;"
        />
        <el-text type="info" size="small" class="mini-hint">
          {{ $t('baseConfig.compressionBrotliLevelHint') }}
        </el-text>
      </el-form-item>
    </template>
  </el-form>
</template>

<script setup lang="ts">
import type { BaseGeneralForm } from './types'

defineProps<{
  model: BaseGeneralForm
}>()
</script>
