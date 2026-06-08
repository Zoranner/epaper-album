<template>
  <button
    class="base-button"
    :class="[variant, { small, block }]"
    :disabled="disabled || loading"
    :type="type"
  >
    <span v-if="loading" class="base-button__spinner" aria-hidden="true"></span>
    <slot>{{ loading ? loadingText : label }}</slot>
  </button>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
    type?: 'button' | 'submit' | 'reset';
    label?: string;
    loadingText?: string;
    disabled?: boolean;
    loading?: boolean;
    small?: boolean;
    block?: boolean;
  }>(),
  {
    variant: 'secondary',
    type: 'button',
    label: '',
    loadingText: '处理中',
    disabled: false,
    loading: false,
    small: false,
    block: false,
  },
);
</script>
