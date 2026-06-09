<template>
  <button
    class="button"
    :class="[variant, { small, block, 'icon-only': iconOnly }]"
    :aria-label="iconOnly ? label : undefined"
    :disabled="disabled || loading"
    :title="iconOnly ? label : undefined"
    :type="type"
  >
    <span v-if="loading" class="button__spinner" aria-hidden="true"></span>
    <Icon v-if="icon && iconPosition === 'left' && !loading" :name="icon" />
    <span v-if="!iconOnly" class="button__label">
      <slot>{{ loading ? loadingText : label }}</slot>
    </span>
    <Icon v-if="icon && iconPosition === 'right' && !loading" :name="icon" />
  </button>
</template>

<script setup lang="ts">
import Icon, { type IconName } from '../display/Icon.vue';

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
    icon?: IconName;
    iconPosition?: 'left' | 'right';
    iconOnly?: boolean;
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
    icon: undefined,
    iconPosition: 'left',
    iconOnly: false,
  },
);
</script>
