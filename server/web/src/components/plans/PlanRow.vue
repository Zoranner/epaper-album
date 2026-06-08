<template>
  <article class="plan-row">
    <div class="plan-row__range">{{ dateRange }}</div>
    <div class="plan-row__main">
      <strong>{{ plan.caption }}</strong>
    </div>
    <div class="plan-strip">
      <div v-if="planImage" class="plan-thumb" :class="planImage.status">
        <img v-if="previewUrls[planImage.sha256]" :src="previewUrls[planImage.sha256]" :alt="planImage.sha256" />
        <span v-else>{{ planImage.status === 'failed' ? '失败' : '处理中' }}</span>
      </div>
      <div v-else class="plan-thumb empty">未选</div>
    </div>
    <div class="plan-row__meta">
      <span>{{ planImage ? 1 : 0 }} 张</span>
    </div>
    <div class="plan-row__actions">
      <BaseActionMenu :items="menuItems" @select="selectAction" />
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { AdminPlan } from '../../api';
import BaseActionMenu, { type BaseActionMenuItem } from '../base/BaseActionMenu.vue';

const props = defineProps<{
  plan: AdminPlan;
  previewUrls: Record<string, string>;
}>();

const emit = defineEmits<{
  editPlan: [];
  deletePlan: [];
}>();

const planImage = computed(() => props.plan.images[0] ?? null);
const dateRange = computed(() => {
  if (props.plan.start === props.plan.end) {
    return formatDate(props.plan.start);
  }
  return `${formatDate(props.plan.start)} 至 ${formatDate(props.plan.end)}`;
});
const menuItems: BaseActionMenuItem[] = [
  { key: 'edit', label: '编辑', icon: 'edit' },
  { key: 'delete', label: '删除', icon: 'trash', danger: true },
];

function selectAction(key: string) {
  if (key === 'edit') {
    emit('editPlan');
  }
  if (key === 'delete') {
    emit('deletePlan');
  }
}

function formatDate(date: string) {
  return date.length >= 10 ? date.slice(5, 10) : date;
}
</script>
