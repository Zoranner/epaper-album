<template>
  <div ref="rootRef" class="field date-picker" :class="{ 'has-error': error, small }">
    <span v-if="label" class="field__label">{{ label }}</span>
    <button
      class="field__control date-picker__trigger"
      :aria-expanded="open"
      :disabled="disabled"
      type="button"
      @click="togglePanel"
      @keydown.esc="closePanel"
    >
      <span :class="{ placeholder: !modelValue }">{{ displayValue }}</span>
      <Icon name="calendar" />
    </button>
    <div v-if="open" class="date-picker__panel">
      <div class="date-picker__nav">
        <Button icon="chevron-left" icon-only label="上个月" small variant="ghost" @click="shiftMonth(-1)" />
        <strong>{{ viewYear }} 年 {{ viewMonth + 1 }} 月</strong>
        <Button icon="chevron-right" icon-only label="下个月" small variant="ghost" @click="shiftMonth(1)" />
      </div>
      <div class="date-picker__weekdays">
        <span v-for="weekday in weekdays" :key="weekday">{{ weekday }}</span>
      </div>
      <div class="date-picker__days">
        <button
          v-for="day in days"
          :key="day.key"
          :class="{ muted: !day.currentMonth, today: day.value === today, selected: day.value === modelValue }"
          type="button"
          @click="selectDate(day.value)"
        >
          {{ day.label }}
        </button>
      </div>
      <div class="date-picker__footer">
        <Button label="今天" small type="button" variant="ghost" @click="selectDate(today)" />
      </div>
    </div>
    <span v-if="error" class="field__error">{{ error }}</span>
    <span v-else-if="hint" class="field__hint">{{ hint }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import Button from '../base/Button.vue';
import Icon from '../display/Icon.vue';

interface CalendarDay {
  key: string;
  label: number;
  value: string;
  currentMonth: boolean;
}

const props = withDefaults(
  defineProps<{
    modelValue: string;
    label?: string;
    hint?: string;
    error?: string;
    disabled?: boolean;
    required?: boolean;
    small?: boolean;
  }>(),
  {
    label: '',
    hint: '',
    error: '',
    disabled: false,
    required: false,
    small: false,
  },
);

const emit = defineEmits<{
  'update:modelValue': [value: string];
}>();

const weekdays = ['一', '二', '三', '四', '五', '六', '日'];
const rootRef = ref<HTMLElement | null>(null);
const open = ref(false);
const current = parseDate(props.modelValue) ?? new Date();
const viewYear = ref(current.getFullYear());
const viewMonth = ref(current.getMonth());
const today = formatDate(new Date());

const displayValue = computed(() => props.modelValue || '请选择日期');

const days = computed<CalendarDay[]>(() => {
  const start = new Date(viewYear.value, viewMonth.value, 1);
  const mondayIndex = (start.getDay() + 6) % 7;
  const cursor = new Date(viewYear.value, viewMonth.value, 1 - mondayIndex);
  return Array.from({ length: 42 }, () => {
    const value = formatDate(cursor);
    const day: CalendarDay = {
      key: value,
      label: cursor.getDate(),
      value,
      currentMonth: cursor.getMonth() === viewMonth.value,
    };
    cursor.setDate(cursor.getDate() + 1);
    return day;
  });
});

function togglePanel() {
  if (props.disabled) {
    return;
  }
  open.value = !open.value;
}

function closePanel() {
  open.value = false;
}

function shiftMonth(offset: number) {
  const next = new Date(viewYear.value, viewMonth.value + offset, 1);
  viewYear.value = next.getFullYear();
  viewMonth.value = next.getMonth();
}

function selectDate(value: string) {
  emit('update:modelValue', value);
  closePanel();
}

function parseDate(value: string) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) {
    return null;
  }
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
}

function formatDate(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function closeOnOutsidePointer(event: PointerEvent) {
  if (!rootRef.value?.contains(event.target as Node)) {
    closePanel();
  }
}

watch(
  () => props.modelValue,
  (value) => {
    const date = parseDate(value);
    if (date) {
      viewYear.value = date.getFullYear();
      viewMonth.value = date.getMonth();
    }
  },
);

watch(open, (isOpen) => {
  if (isOpen) {
    document.addEventListener('pointerdown', closeOnOutsidePointer);
  } else {
    document.removeEventListener('pointerdown', closeOnOutsidePointer);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', closeOnOutsidePointer);
});
</script>
