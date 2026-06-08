<template>
  <section class="plan-picker">
    <div class="plan-picker__toolbar">
      <strong>选择图片</strong>
      <input v-model.trim="keyword" placeholder="搜索备注或 sha256" />
    </div>
    <BaseEmpty v-if="filteredImages.length === 0" small>暂无可选图片</BaseEmpty>
    <div v-else class="picker-grid">
      <button
        v-for="image in filteredImages"
        :key="image.sha256"
        class="picker-tile"
        :class="{ selected: selected.includes(image.sha256), [image.status]: true }"
        type="button"
        @click="$emit('toggle', image.sha256)"
      >
        <img v-if="previewUrls[image.sha256]" :src="previewUrls[image.sha256]" :alt="image.sha256" />
        <span v-else>{{ statusText(image.status) }}</span>
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import type { AdminImage, ImageStatus } from '../../api';
import BaseEmpty from '../base/BaseEmpty.vue';

const props = defineProps<{
  images: AdminImage[];
  selected: string[];
  previewUrls: Record<string, string>;
}>();

defineEmits<{
  toggle: [sha256: string];
}>();

const keyword = ref('');

const filteredImages = computed(() => {
  const term = keyword.value.toLowerCase();
  if (!term) {
    return props.images;
  }
  return props.images.filter(
    (image) => image.sha256.toLowerCase().includes(term) || image.remark.toLowerCase().includes(term),
  );
});

function statusText(status: ImageStatus) {
  if (status === 'ready') {
    return '可显示';
  }
  if (status === 'failed') {
    return '失败';
  }
  return '处理中';
}
</script>
