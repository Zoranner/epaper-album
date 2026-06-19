<template>
  <div class="plan-calendar">
    <PlanMonthGrid
      :days="days"
      :month-title="monthTitle"
      :selected-date="selectedDate"
      :today="today"
      @select-day="selectDay"
      @shift-month="shiftMonth"
    />
    <PlanDetailPanel
      class="plan-calendar__detail"
      :menu-items="detailMenuItems"
      :plan="selectedPlan"
      :preview-urls="previewUrls"
      :selected-date="selectedDate"
      @action="selectDetailAction"
    />
    <PlanMobileAgenda
      :days="days"
      :menu-items="detailMenuItems"
      :month-title="monthTitle"
      :plan="selectedPlan"
      :preview-urls="previewUrls"
      :selected-date="selectedDate"
      :today="today"
      @action="selectDetailAction"
      @select-day="selectDay"
      @shift-month="shiftMonth"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { type ActionMenuItem } from '../navigation/ActionMenu.vue';
import PlanDetailPanel from './PlanDetailPanel.vue';
import PlanMobileAgenda from './PlanMobileAgenda.vue';
import PlanMonthGrid from './PlanMonthGrid.vue';
import type { CalendarDay, PlanView } from './types';

const props = defineProps<{
  month: string;
  plans: PlanView[];
  previewUrls: Record<string, string>;
}>();

const emit = defineEmits<{
  'update:month': [month: string];
  createPlan: [date: string];
  editPlan: [plan: PlanView];
  deletePlan: [plan: PlanView];
}>();

const menuItems: ActionMenuItem[] = [
  { key: 'set', label: '设置', icon: 'edit' },
  { key: 'delete', label: '删除', icon: 'trash', danger: true },
];
const today = toDateString(new Date());
const selectedDate = ref(today);
const monthTitle = computed(() => {
  const date = parseMonth(props.month);
  return `${date.getFullYear()} 年 ${date.getMonth() + 1} 月`;
});

const days = computed<CalendarDay[]>(() => {
  const monthStart = parseMonth(props.month);
  const monthEnd = new Date(monthStart);
  monthEnd.setMonth(monthStart.getMonth() + 1);
  monthEnd.setDate(0);
  const firstWeekday = (monthStart.getDay() + 6) % 7;
  const lastWeekday = (monthEnd.getDay() + 6) % 7;
  const totalDays = firstWeekday + monthEnd.getDate() + (6 - lastWeekday);
  const gridStart = new Date(monthStart);
  gridStart.setDate(monthStart.getDate() - firstWeekday);
  const planByDate = new Map(props.plans.map((plan) => [plan.date, plan]));

  return Array.from({ length: totalDays }, (_, index) => {
    const date = new Date(gridStart);
    date.setDate(gridStart.getDate() + index);
    const dateText = toDateString(date);
    return {
      date: dateText,
      day: String(date.getDate()),
      inMonth: date.getMonth() === monthStart.getMonth(),
      plan: planByDate.get(dateText) ?? null,
    };
  });
});

const selectedDay = computed(() => days.value.find((day) => day.date === selectedDate.value) ?? null);
const selectedPlan = computed(() => selectedDay.value?.plan ?? null);
const detailMenuItems = computed<ActionMenuItem[]>(() => {
  return menuItems.map((item) => ({
    ...item,
    disabled: item.key === 'delete' && !selectedPlan.value,
  }));
});

watch(
  () => props.month,
  () => {
    const monthStart = parseMonth(props.month);
    if (selectedDate.value.slice(0, 7) !== props.month) {
      selectedDate.value = toDateString(monthStart);
    }
  },
);

function selectDay(day: CalendarDay) {
  selectedDate.value = day.date;
}

function selectDetailAction(key: string) {
  if (key === 'set' && !selectedPlan.value) {
    emit('createPlan', selectedDate.value);
    return;
  }
  if (!selectedPlan.value) {
    return;
  }
  if (key === 'set') {
    emit('editPlan', selectedPlan.value);
  }
  if (key === 'delete') {
    emit('deletePlan', selectedPlan.value);
  }
}

function shiftMonth(offset: number) {
  const date = parseMonth(props.month);
  date.setMonth(date.getMonth() + offset);
  emit('update:month', `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`);
}

function toDateString(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function parseMonth(month: string) {
  const [year, monthNumber] = month.split('-').map(Number);
  return new Date(year, monthNumber - 1, 1);
}
</script>
