<template>
  <article class="image-tile" :class="image.status">
    <div class="image-tile__preview">
      <img v-if="previewUrl" :src="previewUrl" :alt="image.sha256" />
      <span v-else>{{ statusText }}</span>
    </div>
    <div class="image-tile__body">
      <p>{{ image.remark || '未填写备注' }}</p>
      <code :title="image.sha256">{{ shortSha(image.sha256) }}</code>
    </div>
    <div class="image-tile__footer">
      <span>{{ statusText }}</span>
      <div class="tile-menu">
        <button type="button">...</button>
        <div class="tile-menu__items">
          <button type="button" @click="$emit('editRemark')">编辑备注</button>
          <button v-if="image.status === 'ready'" type="button" @click="$emit('refreshPreview')">
            刷新预览
          </button>
        </div>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { AdminImage } from '../../api';

const props = defineProps<{
  image: AdminImage;
  previewUrl?: string;
}>();

defineEmits<{
  editRemark: [];
  refreshPreview: [];
}>();

const statusText = computed(() => {
  if (props.image.status === 'ready') {
    return '可显示';
  }
  if (props.image.status === 'failed') {
    return '处理失败';
  }
  if (props.image.status === 'processing') {
    return '处理中';
  }
  return '待处理';
});

function shortSha(sha256: string) {
  return sha256.length > 16 ? `${sha256.slice(0, 8)}...${sha256.slice(-6)}` : sha256;
}
</script>
