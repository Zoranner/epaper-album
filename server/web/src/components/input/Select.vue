<template>
  <label class="field" :class="{ 'has-error': error, small }">
    <span v-if="label" class="field__label">{{ label }}</span>
    <span class="select">
      <select
        class="field__control"
        :disabled="disabled"
        :required="required"
        :value="modelValue"
        @change="updateValue"
      >
        <option v-for="option in options" :key="option.value" :value="option.value">
          {{ option.label }}
        </option>
      </select>
      <Icon name="chevron-down" />
    </span>
    <span v-if="error" class="field__error">{{ error }}</span>
    <span v-else-if="hint" class="field__hint">{{ hint }}</span>
  </label>
</template>

<script setup lang="ts">
import Icon from '../display/Icon.vue';

export interface SelectOption {
  label: string;
  value: string;
}

withDefaults(
  defineProps<{
    modelValue: string;
    options: SelectOption[];
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
