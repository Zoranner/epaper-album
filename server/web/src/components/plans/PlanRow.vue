<template>
  <article class="plan-row">
    <div class="plan-row__header">
      <div>
        <div class="plan-row__date">{{ formattedDate }}</div>
        <div class="plan-row__main">
          <strong>{{ plan.caption }}</strong>
        </div>
      </div>
      <div class="plan-row__actions">
        <ActionMenu :items="menuItems" @select="selectAction" />
      </div>
    </div>
    <div class="plan-row__content">
      <div v-if="planImage" class="plan-thumb" :class="planImage.status">
        <img v-if="previewUrls[planImage.sha256]" :src="previewUrls[planImage.sha256]" :alt="planImage.sha256" />
        <span v-else>{{ planImage.status === 'failed' ? '失败' : '处理中' }}</span>
      </div>
      <div v-else class="plan-thumb empty">未选</div>
      <div class="plan-row__meta">
        <span>图片 SHA256</span>
        <code>{{ plan.image || '-' }}</code>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import ActionMenu, { type ActionMenuItem } from '../navigation/ActionMenu.vue';
import type { PlanView } from './types';

const props = defineProps<{
  plan: PlanView;
  previewUrls: Record<string, string>;
}>();

const emit = defineEmits<{
  editPlan: [];
  deletePlan: [];
}>();

const planImage = computed(() => props.plan.imageRecord ?? null);
const formattedDate = computed(() => formatDate(props.plan.date));
const menuItems: ActionMenuItem[] = [
  { key: 'edit', label: '编辑', icon: 'edit' },
  { key: 'delete', label: '删除', icon: 'trash', danger: true },
];

function selectAction(key: string) {
  if (key === 'edit') {
    emit('editPlan');
  }
  if (key === 'delete') {
    emit('deletePlan');
  }
}

function formatDate(date: string) {
  return date.length >= 10 ? date.slice(5, 10) : date;
}
</script>
