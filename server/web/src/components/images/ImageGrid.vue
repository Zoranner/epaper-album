<template>
  <EmptyState v-if="images.length === 0" small>暂无图片</EmptyState>
  <div v-else class="image-grid">
    <ImageTile
      v-for="image in images"
      :key="image.sha256"
      :image="image"
      :preview-url="previewUrls[image.sha256]"
      @edit-remark="$emit('editRemark', image)"
      @refresh-preview="$emit('refreshPreview', image.sha256)"
    />
  </div>
</template>

<script setup lang="ts">
import type { AdminImage } from '../../api';
import EmptyState from '../feedback/EmptyState.vue';
import ImageTile from './ImageTile.vue';

defineProps<{
  images: AdminImage[];
  previewUrls: Record<string, string>;
}>();

defineEmits<{
  editRemark: [image: AdminImage];
  refreshPreview: [sha256: string];
}>();
</script>
