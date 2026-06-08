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
      <BaseActionMenu :items="menuItems" @select="selectAction" />
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { AdminImage } from '../../api';
import BaseActionMenu, { type BaseActionMenuItem } from '../base/BaseActionMenu.vue';

const props = defineProps<{
  image: AdminImage;
  previewUrl?: string;
}>();

const emit = defineEmits<{
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
const menuItems = computed<BaseActionMenuItem[]>(() => {
  const items: BaseActionMenuItem[] = [{ key: 'edit', label: '编辑备注', icon: 'edit' }];
  if (props.image.status === 'ready') {
    items.push({ key: 'refresh', label: '刷新预览', icon: 'refresh' });
  }
  return items;
});

function selectAction(key: string) {
  if (key === 'edit') {
    emit('editRemark');
  }
  if (key === 'refresh') {
    emit('refreshPreview');
  }
}

function shortSha(sha256: string) {
  return sha256.length > 16 ? `${sha256.slice(0, 8)}...${sha256.slice(-6)}` : sha256;
}
</script>
