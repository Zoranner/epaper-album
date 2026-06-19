<template>
  <div class="meta-item">
    <dt>{{ label }}</dt>
    <dd><slot></slot></dd>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  label: string;
}>();
</script>
