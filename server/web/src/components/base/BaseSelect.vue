<template>
  <label class="base-field" :class="{ 'has-error': error, small }">
    <span v-if="label" class="base-field__label">{{ label }}</span>
    <span class="base-select">
      <select
        class="base-field__control"
        :disabled="disabled"
        :required="required"
        :value="modelValue"
        @change="updateValue"
      >
        <option v-for="option in options" :key="option.value" :value="option.value">
          {{ option.label }}
        </option>
      </select>
      <BaseIcon name="chevron-down" />
    </span>
    <span v-if="error" class="base-field__error">{{ error }}</span>
    <span v-else-if="hint" class="base-field__hint">{{ hint }}</span>
  </label>
</template>

<script setup lang="ts">
import BaseIcon from './BaseIcon.vue';

export interface BaseSelectOption {
  label: string;
  value: string;
}

withDefaults(
  defineProps<{
    modelValue: string;
    options: BaseSelectOption[];
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

function updateValue(event: Event) {
  emit('update:modelValue', (event.target as HTMLSelectElement).value);
}
</script>
