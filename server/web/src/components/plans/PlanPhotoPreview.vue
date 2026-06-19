<template>
  <div v-if="!plan" class="plan-photo-preview empty" title="未设置计划">
    <span class="plan-photo-preview__text">未设置</span>
  </div>
  <div v-else-if="plan.type === 'random'" class="plan-photo-preview random" :title="title">
    <span class="plan-photo-preview__text">{{ text }}</span>
    <span class="plan-photo-preview__badge">随机</span>
  </div>
  <div v-else-if="plan.imageRecord" class="plan-photo-preview" :class="plan.imageRecord.status" :title="title">
    <img v-if="previewUrls[plan.imageRecord.sha256]" :src="previewUrls[plan.imageRecord.sha256]" :alt="imageAlt" />
    <StatusBadge v-else :status="plan.imageRecord.status" />
    <span class="plan-photo-preview__badge">固定</span>
  </div>
  <div v-else class="plan-photo-preview empty" :title="text">
    未选
    <span class="plan-photo-preview__badge">固定</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import StatusBadge from '../display/StatusBadge.vue';
import type { PlanView } from './types';

const props = defineProps<{
  plan: PlanView | null;
  previewUrls: Record<string, string>;
}>();

const title = computed(() => {
  if (!props.plan) {
    return '未设置计划';
  }
  if (props.plan.type === 'random') {
    return props.plan.tags?.join(' ') ?? '';
  }
  return props.plan.imageRecord?.remark ?? '';
});

const text = computed(() => {
  if (!props.plan) {
    return '未设置';
  }
  if (props.plan.type === 'random') {
    return props.plan.tags?.length ? props.plan.tags.slice(0, 3).join(' / ') : '未设置标签';
  }
  if (props.plan.imageRecord?.remark) {
    return props.plan.imageRecord.remark;
  }
  if (props.plan.imageRecord) {
    return '固定照片';
  }
  return props.plan.image ? '照片不可用' : '未选照片';
});

const imageAlt = computed(() => {
  if (!props.plan) {
    return '计划照片';
  }
  return props.plan.imageRecord?.remark || props.plan.caption || '计划照片';
});
</script>
