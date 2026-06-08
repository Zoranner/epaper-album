<template>
  <BaseInput
    :disabled="disabled"
    :error="error"
    :hint="hint"
    :label="label"
    :model-value="modelValue"
    :required="required"
    type="date"
    @update:model-value="$emit('update:modelValue', $event)"
  />
</template>

<script setup lang="ts">
import BaseInput from './BaseInput.vue';

withDefaults(
  defineProps<{
    modelValue: string;
    label?: string;
    hint?: string;
    error?: string;
    disabled?: boolean;
    required?: boolean;
  }>(),
  {
    label: '',
    hint: '',
    error: '',
    disabled: false,
    required: false,
  },
);

defineEmits<{
  'update:modelValue': [value: string];
}>();
</script>
