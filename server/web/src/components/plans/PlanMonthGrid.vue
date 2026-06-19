<template>
  <section class="plan-month">
    <header class="plan-month__nav">
      <button type="button" aria-label="上个月" @click="$emit('shiftMonth', -1)">
        <Icon name="chevron-left" />
      </button>
      <strong>{{ monthTitle }}</strong>
      <button type="button" aria-label="下个月" @click="$emit('shiftMonth', 1)">
        <Icon name="chevron-right" />
      </button>
    </header>
    <div class="plan-month__weekdays">
      <span v-for="weekday in weekdays" :key="weekday">{{ weekday }}</span>
    </div>
    <div class="plan-month__grid">
      <button
        v-for="day in days"
        :key="day.date"
        class="plan-day"
        :class="{
          muted: !day.inMonth,
          planned: Boolean(day.plan),
          today: day.date === today,
          selected: day.date === selectedDate,
        }"
        type="button"
        @click="$emit('selectDay', day)"
      >
        <span>{{ day.day }}</span>
        <strong v-if="day.plan">{{ day.plan.caption }}</strong>
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
import Icon from '../display/Icon.vue';
import type { CalendarDay } from './types';

defineProps<{
  days: CalendarDay[];
  monthTitle: string;
  selectedDate: string;
  today: string;
}>();

defineEmits<{
  selectDay: [day: CalendarDay];
  shiftMonth: [offset: number];
}>();

const weekdays = ['一', '二', '三', '四', '五', '六', '日'];
</script>
