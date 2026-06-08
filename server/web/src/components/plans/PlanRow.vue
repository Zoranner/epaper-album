<template>
  <article class="plan-row">
    <div class="plan-row__main">
      <strong>{{ plan.caption }}</strong>
      <span>{{ plan.start }} 至 {{ plan.end }}</span>
    </div>
    <div class="plan-strip">
      <div v-for="image in visibleImages" :key="image.sha256" class="plan-thumb" :class="image.status">
        <img v-if="previewUrls[image.sha256]" :src="previewUrls[image.sha256]" :alt="image.sha256" />
        <span v-else>{{ image.status === 'failed' ? '失败' : '处理中' }}</span>
      </div>
      <div v-if="hiddenCount > 0" class="plan-thumb more">+{{ hiddenCount }}</div>
    </div>
    <div class="plan-row__meta">
      <span>{{ plan.images.length }} 张</span>
      <div class="tile-menu">
        <button type="button">...</button>
        <div class="tile-menu__items">
          <button type="button" @click="$emit('editPlan')">编辑</button>
          <button type="button" @click="$emit('deletePlan')">删除</button>
        </div>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { AdminPlan } from '../../api';

const props = defineProps<{
  plan: AdminPlan;
  previewUrls: Record<string, string>;
}>();

defineEmits<{
  editPlan: [];
  deletePlan: [];
}>();

const visibleImages = computed(() => props.plan.images.slice(0, 6));
const hiddenCount = computed(() => Math.max(0, props.plan.images.length - visibleImages.value.length));
</script>
