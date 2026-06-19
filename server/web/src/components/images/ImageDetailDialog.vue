<template>
  <Dialog :open="open" title="图片详情" @close="$emit('close')">
    <template #actions>
      <ActionMenu v-if="image" :items="menuItems" @select="selectAction" />
    </template>
    <div v-if="image" class="image-detail">
      <div class="image-detail__preview">
        <img v-if="previewUrl" :src="previewUrl" :alt="image.sha256" />
        <span v-else>{{ image.status === 'ready' ? '无预览' : '暂无预览' }}</span>
      </div>
      <div class="image-detail__panel">
        <div class="image-detail__header">
          <p>{{ image.remark || '未填写备注' }}</p>
          <StatusBadge :status="image.status" />
        </div>

        <MetaList class="image-detail__meta">
          <MetaItem label="创建时间">{{ formatFullImageTime(image.createdAt) }}</MetaItem>
          <MetaItem label="更新时间">{{ formatFullImageTime(image.updatedAt) }}</MetaItem>
          <MetaItem label="SHA256">
            <code>{{ image.sha256 }}</code>
          </MetaItem>
          <MetaItem label="标签">
            <div class="image-detail__tag-box">
              <div v-if="image.tags.length > 0" class="tag-list image-detail__tags">
                <span v-for="tag in image.tags" :key="tag" class="tag-chip">{{ tag }}</span>
              </div>
              <span v-else>无标签</span>
            </div>
          </MetaItem>
        </MetaList>
      </div>
    </div>
  </Dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { AdminImage } from '../../api';
import MetaItem from '../display/MetaItem.vue';
import MetaList from '../display/MetaList.vue';
import StatusBadge from '../display/StatusBadge.vue';
import ActionMenu, { type ActionMenuItem } from '../navigation/ActionMenu.vue';
import Dialog from '../overlay/Dialog.vue';
import { formatFullImageTime } from './imageDisplay';

const props = defineProps<{
  open: boolean;
  image: AdminImage | null;
  previewUrl?: string;
}>();

const menuItems = computed<ActionMenuItem[]>(() => {
  if (!props.image) {
    return [];
  }
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

const emit = defineEmits<{
  close: [];
  editImage: [];
  refreshPreview: [];
  reditherImage: [];
  deleteImage: [];
}>();

function selectAction(key: string) {
  if (key === 'edit') {
    emit('editImage');
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
