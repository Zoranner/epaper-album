<template>
  <label class="base-field" :class="{ 'has-error': error }">
    <span v-if="label" class="base-field__label">{{ label }}</span>
    <span class="base-file-input__row">
      <input
        class="base-file-input__native"
        :accept="accept"
        :disabled="disabled"
        :required="required"
        type="file"
        @change="updateFile"
      />
      <span class="base-button secondary small" aria-hidden="true">{{ buttonText }}</span>
      <span class="base-file-input__name">{{ fileName || placeholder }}</span>
    </span>
    <span v-if="error" class="base-field__error">{{ error }}</span>
    <span v-else-if="hint" class="base-field__hint">{{ hint }}</span>
  </label>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    label?: string;
    hint?: string;
    error?: string;
    accept?: string;
    fileName?: string;
    buttonText?: string;
    placeholder?: string;
    disabled?: boolean;
    required?: boolean;
  }>(),
  {
    label: '',
    hint: '',
    error: '',
    accept: '',
    fileName: '',
    buttonText: '选择文件',
    placeholder: '未选择文件',
    disabled: false,
    required: false,
  },
);

const emit = defineEmits<{
  'update:file': [file: File | null];
  select: [file: File | null];
}>();

function updateFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0] ?? null;
  emit('update:file', file);
  emit('select', file);
}
</script>
