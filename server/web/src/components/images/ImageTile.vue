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
      <ActionMenu :items="menuItems" @select="selectAction" />
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { AdminImage } from '../../api';
import ActionMenu, { type ActionMenuItem } from '../navigation/ActionMenu.vue';

const props = defineProps<{
  image: AdminImage;
  previewUrl?: string;
}>();

const emit = defineEmits<{
  editRemark: [];
  refreshPreview: [];
  deleteImage: [];
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
const menuItems = computed<ActionMenuItem[]>(() => {
  const items: ActionMenuItem[] = [{ key: 'edit', label: '编辑备注', icon: 'edit' }];
  if (props.image.status === 'ready') {
    items.push({ key: 'refresh', label: '刷新预览', icon: 'refresh' });
  }
  items.push({ key: 'delete', label: '删除图片', icon: 'trash', danger: true });
  return items;
});

function selectAction(key: string) {
  if (key === 'edit') {
    emit('editRemark');
  }
  if (key === 'refresh') {
    emit('refreshPreview');
  }
  if (key === 'delete') {
    emit('deleteImage');
  }
}

function shortSha(sha256: string) {
  return sha256.length > 16 ? `${sha256.slice(0, 8)}...${sha256.slice(-6)}` : sha256;
}
</script>
