<template>
  <span class="status-badge" :class="status">{{ text }}</span>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ImageStatus } from '../../api';

const props = defineProps<{
  status: ImageStatus;
}>();

const text = computed(() => {
  if (props.status === 'ready') {
    return '已处理';
  }
  if (props.status === 'failed') {
    return '失败';
  }
  if (props.status === 'processing') {
    return '处理中';
  }
  return '待处理';
});
</script>
