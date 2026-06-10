<template>
  <label class="field" :class="{ 'has-error': error, small }">
    <span v-if="label" class="field__label">{{ label }}</span>
    <input
      class="field__control"
      :autocomplete="autocomplete"
      :disabled="disabled"
      :maxlength="maxlength"
      :placeholder="placeholder"
      :required="required"
      :type="type"
      :value="modelValue"
      @input="updateValue"
    />
    <span v-if="error" class="field__error">{{ error }}</span>
    <span v-else-if="hint" class="field__hint">{{ hint }}</span>
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
    type?: 'text' | 'password' | 'search';
    autocomplete?: string;
    maxlength?: number | string;
    disabled?: boolean;
    required?: boolean;
    small?: boolean;
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
    small: false,
  },
);

const emit = defineEmits<{
  'update:modelValue': [value: string];
}>();

function updateValue(event: Event) {
  emit('update:modelValue', (event.target as HTMLInputElement).value);
}
</script>
