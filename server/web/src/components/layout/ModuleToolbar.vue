<template>
  <header class="module-toolbar">
    <div class="module-toolbar__text">
      <h2>{{ title }}</h2>
      <p v-if="description">{{ description }}</p>
    </div>
    <div class="module-actions">
      <slot></slot>
    </div>
  </header>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string;
    description?: string;
  }>(),
  {
    description: '',
  },
);
</script>
