<template>
  <EmptyState v-if="plans.length === 0" small>暂无计划</EmptyState>
  <div v-else class="plan-table">
    <div class="plan-table__head">
      <span>日期</span>
      <span>标题</span>
      <span>图片</span>
      <span>图片 SHA256</span>
      <span>操作</span>
    </div>
    <PlanRow
      v-for="plan in plans"
      :key="plan.date"
      :plan="plan"
      :preview-urls="previewUrls"
      @delete-plan="$emit('deletePlan', plan)"
      @edit-plan="$emit('editPlan', plan)"
    />
  </div>
</template>

<script setup lang="ts">
import EmptyState from '../feedback/EmptyState.vue';
import PlanRow from './PlanRow.vue';
import type { PlanView } from './types';

defineProps<{
  plans: PlanView[];
  previewUrls: Record<string, string>;
}>();

defineEmits<{
  editPlan: [plan: PlanView];
  deletePlan: [plan: PlanView];
}>();
</script>
