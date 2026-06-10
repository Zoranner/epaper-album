<template>
  <EmptyState v-if="plans.length === 0" small>暂无计划</EmptyState>
  <div v-else class="plan-table">
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
