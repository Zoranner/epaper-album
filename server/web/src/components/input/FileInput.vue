<template>
  <label class="field" :class="{ 'has-error': error }">
    <span v-if="label" class="field__label">{{ label }}</span>
    <span class="file-input__row">
      <input
        class="file-input__native"
        :accept="accept"
        :disabled="disabled"
        :required="required"
        type="file"
        @change="updateFile"
      />
      <span class="button secondary small" aria-hidden="true">{{ buttonText }}</span>
      <span class="file-input__name">{{ fileName || placeholder }}</span>
    </span>
    <span v-if="error" class="field__error">{{ error }}</span>
    <span v-else-if="hint" class="field__hint">{{ hint }}</span>
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
