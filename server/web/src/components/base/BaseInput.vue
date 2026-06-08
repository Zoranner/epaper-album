<template>
  <label class="base-field" :class="{ 'has-error': error }">
    <span v-if="label" class="base-field__label">{{ label }}</span>
    <input
      class="base-field__control"
      :autocomplete="autocomplete"
      :disabled="disabled"
      :maxlength="maxlength"
      :placeholder="placeholder"
      :required="required"
      :type="type"
      :value="modelValue"
      @input="updateValue"
    />
    <span v-if="error" class="base-field__error">{{ error }}</span>
    <span v-else-if="hint" class="base-field__hint">{{ hint }}</span>
  </label>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    modelValue: string;
    label?: string;
    hint?: string;
    error?: string;
    placeholder?: string;
    type?: 'text' | 'password' | 'search' | 'date';
    autocomplete?: string;
    maxlength?: number | string;
    disabled?: boolean;
    required?: boolean;
  }>(),
  {
    label: '',
    hint: '',
    error: '',
    placeholder: '',
    type: 'text',
    autocomplete: undefined,
    maxlength: undefined,
    disabled: false,
    required: false,
  },
);

const emit = defineEmits<{
  'update:modelValue': [value: string];
}>();

function updateValue(event: Event) {
  emit('update:modelValue', (event.target as HTMLInputElement).value);
}
</script>
