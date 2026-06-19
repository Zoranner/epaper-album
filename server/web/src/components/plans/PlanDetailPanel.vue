<template>
  <section class="plan-detail-panel">
    <header class="plan-detail-panel__header">
      <div>
        <time :datetime="selectedDate">{{ detailDateText(selectedDate) }}</time>
        <h3>{{ plan?.caption ?? '未设置计划' }}</h3>
      </div>
      <ActionMenu :items="menuItems" @select="$emit('action', $event)" />
    </header>
    <div class="plan-detail-panel__photo">
      <PlanPhotoPreview :plan="plan" :preview-urls="previewUrls" />
    </div>
  </section>
</template>

<script setup lang="ts">
import ActionMenu, { type ActionMenuItem } from '../navigation/ActionMenu.vue';
import PlanPhotoPreview from './PlanPhotoPreview.vue';
import type { PlanView } from './types';

defineProps<{
  selectedDate: string;
  plan: PlanView | null;
  previewUrls: Record<string, string>;
  menuItems: ActionMenuItem[];
}>();

defineEmits<{
  action: [key: string];
}>();

function detailDateText(value: string) {
  const date = new Date(`${value}T00:00:00`);
  return `${date.getFullYear()} 年 ${date.getMonth() + 1} 月 ${date.getDate()} 日`;
}
</script>
