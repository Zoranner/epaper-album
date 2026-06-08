<template>
  <label class="base-field" :class="{ 'has-error': error, small }">
    <span v-if="label" class="base-field__label">{{ label }}</span>
    <input
      class="base-field__control"
      :disabled="disabled"
      :max="max"
      :min="min"
      :placeholder="placeholder"
      :required="required"
      :step="step"
      type="number"
      :value="displayValue"
      @change="$emit('change')"
      @input="updateValue"
    />
    <span v-if="error" class="base-field__error">{{ error }}</span>
    <span v-else-if="hint" class="base-field__hint">{{ hint }}</span>
  </label>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    modelValue: number | null;
    label?: string;
    hint?: string;
    error?: string;
    placeholder?: string;
    min?: number | string;
    max?: number | string;
    step?: number | string;
    disabled?: boolean;
    required?: boolean;
    small?: boolean;
  }>(),
  {
    label: '',
    hint: '',
    error: '',
    placeholder: '',
    min: undefined,
    max: undefined,
    step: 1,
    disabled: false,
    required: false,
    small: false,
  },
);

const emit = defineEmits<{
  'update:modelValue': [value: number];
  change: [];
}>();

const displayValue = computed(() => (props.modelValue === null ? '' : String(props.modelValue)));

function updateValue(event: Event) {
  const value = (event.target as HTMLInputElement).value;
  emit('update:modelValue', value === '' ? Number(props.min ?? 0) : Number(value));
}
</script>
