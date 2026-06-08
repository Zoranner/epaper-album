<template>
  <button
    class="base-icon-button"
    :aria-label="label"
    :disabled="disabled"
    :title="label"
    :type="type"
    @click="$emit('click')"
  >
    <BaseIcon :name="icon" />
  </button>
</template>

<script setup lang="ts">
import BaseIcon from './BaseIcon.vue';

withDefaults(
defineProps<{
    icon: InstanceType<typeof BaseIcon>['$props']['name'];
    label: string;
    type?: 'button' | 'submit' | 'reset';
    disabled?: boolean;
}>(),
  {
    type: 'button',
    disabled: false,
  },
);

defineEmits<{
  click: [];
}>();
</script>
