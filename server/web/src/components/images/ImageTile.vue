<template>
  <article class="image-tile" :class="image.status">
    <button class="image-tile__preview" type="button" @click="$emit('openDetail')">
      <img v-if="previewUrl" :src="previewUrl" :alt="image.sha256" />
      <span v-else>{{ image.status === 'ready' ? '无预览' : '' }}</span>
      <StatusBadge :status="image.status" />
    </button>
    <button class="image-tile__body" type="button" @click="$emit('openDetail')">
      <p>{{ image.remark || '未填写备注' }}</p>
      <div class="image-tile__meta">
        <span>{{ updatedTime }}</span>
        <span>{{ tagCountText }}</span>
      </div>
    </button>
    <div class="image-tile__footer">
      <span class="image-tile__sha">{{ shortSha }}</span>
      <ActionMenu :items="menuItems" @select="selectAction" />
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { AdminImage } from '../../api';
import StatusBadge from '../display/StatusBadge.vue';
import ActionMenu, { type ActionMenuItem } from '../navigation/ActionMenu.vue';
import { formatImageTime } from './imageDisplay';

const props = defineProps<{
  image: AdminImage;
  previewUrl?: string;
}>();

const emit = defineEmits<{
  openDetail: [];
  editRemark: [];
  refreshPreview: [];
  reditherImage: [];
  deleteImage: [];
}>();

const tagCountText = computed(() => (props.image.tags.length > 0 ? `${props.image.tags.length} 个标签` : '无标签'));
const updatedTime = computed(() => formatImageTime(props.image.updatedAt));
const shortSha = computed(() =>
  props.image.sha256.length > 14 ? `${props.image.sha256.slice(0, 8)}...${props.image.sha256.slice(-4)}` : props.image.sha256,
);
const menuItems = computed<ActionMenuItem[]>(() => {
  const items: ActionMenuItem[] = [{ key: 'edit', label: '编辑信息', icon: 'edit' }];
  if (props.image.status === 'ready') {
    items.push({ key: 'refresh', label: '刷新预览', icon: 'refresh' });
  }
  if (props.image.status === 'ready' || props.image.status === 'failed') {
    items.push({ key: 'redither', label: '重新抖动', icon: 'refresh' });
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
  if (key === 'redither') {
    emit('reditherImage');
  }
  if (key === 'delete') {
    emit('deleteImage');
  }
}
</script>
