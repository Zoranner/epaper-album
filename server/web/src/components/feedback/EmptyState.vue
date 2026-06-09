<template>
  <div class="empty-state" :class="{ small }">
    <p v-if="title" class="empty-state__title">{{ title }}</p>
    <p v-if="description" class="empty-state__description">{{ description }}</p>
    <slot></slot>
  </div>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    title?: string;
    description?: string;
    small?: boolean;
  }>(),
  {
    title: '',
    description: '',
    small: false,
  },
);
</script>
