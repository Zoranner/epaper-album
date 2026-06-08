<template>
  <section class="day-group">
    <header class="day-group__header">
      <h3>{{ date }}</h3>
      <span>{{ plans.length }} 个计划</span>
    </header>
    <BaseEmpty v-if="plans.length === 0" small>暂无计划</BaseEmpty>
    <div v-else class="plan-row-list">
      <PlanRow
        v-for="plan in plans"
        :key="plan.id"
        :plan="plan"
        :preview-urls="previewUrls"
        @delete-plan="$emit('deletePlan', plan)"
        @edit-plan="$emit('editPlan', plan)"
      />
    </div>
  </section>
</template>

<script setup lang="ts">
import type { AdminPlan } from '../../api';
import BaseEmpty from '../base/BaseEmpty.vue';
import PlanRow from './PlanRow.vue';

defineProps<{
  date: string;
  plans: AdminPlan[];
  previewUrls: Record<string, string>;
}>();

defineEmits<{
  editPlan: [plan: AdminPlan];
  deletePlan: [plan: AdminPlan];
}>();
</script>
