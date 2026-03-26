<template>
  <el-form :model="model" label-width="180px">
    <el-form-item :label="$t('about.alertingEnabled')">
      <el-switch v-model="model.enabled" />
    </el-form-item>

    <template v-if="model.enabled">
      <el-form-item :label="$t('about.alertWebhookEnabled')">
        <el-switch v-model="model.webhook.enabled" />
      </el-form-item>

      <template v-if="model.webhook.enabled">
        <el-form-item :label="$t('about.alertProvider')">
          <el-select v-model="model.webhook.provider" style="width: 220px;">
            <el-option label="企业微信 WeCom" value="wecom" />
            <el-option label="飞书 Feishu" value="feishu" />
          </el-select>
        </el-form-item>

        <el-form-item :label="$t('about.alertWebhookUrl')">
          <el-input v-model="model.webhook.url" :placeholder="$t('about.alertWebhookUrlPlaceholder')" />
        </el-form-item>

        <el-divider />

        <el-form-item :label="$t('about.systemReportEnabled')">
          <el-switch v-model="model.webhook.system_report_enabled" />
          <el-text type="info" size="small" class="mini-hint">
            {{ $t('about.systemReportHint') }}
          </el-text>
        </el-form-item>

        <template v-if="model.webhook.system_report_enabled">
          <el-form-item :label="$t('about.systemReportIntervalMinutes')">
            <el-select
              v-model="model.webhook.system_report_interval_minutes"
              filterable
              allow-create
              default-first-option
              style="width: 220px;"
            >
              <el-option
                v-for="item in intervalOptions"
                :key="item"
                :label="String(item)"
                :value="item"
              />
            </el-select>
            <el-text type="info" size="small" class="mini-hint">
              {{ $t('about.systemReportIntervalHint') }}
            </el-text>
          </el-form-item>

          <el-form-item :label="$t('about.systemReportWeekdays')">
            <el-checkbox-group v-model="model.webhook.system_report_weekdays">
              <el-checkbox
                v-for="day in weekdayOptions"
                :key="day.value"
                :label="day.value"
              >
                {{ day.label }}
              </el-checkbox>
            </el-checkbox-group>
          </el-form-item>

          <el-form-item :label="$t('about.quietHoursEnabled')">
            <el-switch v-model="model.webhook.quiet_hours_enabled" />
          </el-form-item>

          <template v-if="model.webhook.quiet_hours_enabled">
            <el-form-item :label="$t('about.quietHoursStart')">
              <el-time-picker
                v-model="model.webhook.quiet_hours_start"
                format="HH:mm"
                value-format="HH:mm"
                :clearable="false"
              />
            </el-form-item>

            <el-form-item :label="$t('about.quietHoursEnd')">
              <el-time-picker
                v-model="model.webhook.quiet_hours_end"
                format="HH:mm"
                value-format="HH:mm"
                :clearable="false"
              />
              <el-text type="info" size="small" class="mini-hint">
                {{ $t('about.quietHoursHint') }}
              </el-text>
            </el-form-item>
          </template>
        </template>
      </template>

      <el-form-item :label="$t('about.alertRuleServerStartError')">
        <el-switch v-model="model.rules.server_start_error" />
      </el-form-item>

      <el-form-item>
        <el-button type="primary" @click="$emit('send-test-alert')" :loading="sendingTestAlert">
          {{ $t('about.sendTestAlert') }}
        </el-button>
      </el-form-item>
    </template>
  </el-form>
</template>

<script setup lang="ts">
import type { AlertingForm } from './types'

defineProps<{
  model: AlertingForm
  sendingTestAlert: boolean
  intervalOptions: number[]
  weekdayOptions: Array<{ value: number, label: string }>
}>()

defineEmits<{
  (e: 'send-test-alert'): void
}>()
</script>
