<template>
  <section class="plan-mobile-agenda">
    <header class="plan-mobile-agenda__nav">
      <button type="button" aria-label="上个月" @click="$emit('shiftMonth', -1)">
        <Icon name="chevron-left" />
      </button>
      <strong>{{ monthTitle }}</strong>
      <button type="button" aria-label="下个月" @click="$emit('shiftMonth', 1)">
        <Icon name="chevron-right" />
      </button>
    </header>
    <div class="plan-mobile-agenda__days">
      <button
        v-for="day in days"
        :key="`mobile-${day.date}`"
        class="plan-mobile-day"
        :class="{
          muted: !day.inMonth,
          planned: Boolean(day.plan),
          today: day.date === today,
          selected: day.date === selectedDate,
        }"
        type="button"
        @click="$emit('selectDay', day)"
      >
        {{ day.day }}
      </button>
    </div>
    <PlanDetailPanel
      :menu-items="menuItems"
      :plan="plan"
      :preview-urls="previewUrls"
      :selected-date="selectedDate"
      @action="$emit('action', $event)"
    />
  </section>
</template>

<script setup lang="ts">
import { type ActionMenuItem } from '../navigation/ActionMenu.vue';
import Icon from '../display/Icon.vue';
import PlanDetailPanel from './PlanDetailPanel.vue';
import type { CalendarDay, PlanView } from './types';

defineProps<{
  days: CalendarDay[];
  monthTitle: string;
  selectedDate: string;
  plan: PlanView | null;
  previewUrls: Record<string, string>;
  menuItems: ActionMenuItem[];
  today: string;
}>();

defineEmits<{
  selectDay: [day: CalendarDay];
  shiftMonth: [offset: number];
  action: [key: string];
}>();
</script>
