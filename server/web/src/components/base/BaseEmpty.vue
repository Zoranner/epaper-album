<template>
  <div class="base-empty" :class="{ small }">
    <p v-if="title" class="base-empty__title">{{ title }}</p>
    <p v-if="description" class="base-empty__description">{{ description }}</p>
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
