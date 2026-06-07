<template>
  <form class="plan-editor" @submit.prevent="submit">
    <div class="section-title compact">
      <div>
        <h2>{{ plan ? '编辑计划' : '新增计划' }}</h2>
        <p>计划图片从已上传图片中选择</p>
      </div>
      <button v-if="plan" class="button ghost" type="button" @click="$emit('cancel')">取消</button>
    </div>

    <div class="form-grid">
      <label>
        <span>开始日期</span>
        <input v-model.trim="draft.start" required type="date" />
      </label>
      <label>
        <span>结束日期</span>
        <input v-model.trim="draft.end" required type="date" />
      </label>
    </div>

    <label>
      <span>标题</span>
      <input v-model.trim="draft.caption" required maxlength="80" placeholder="例如：晚风和海" />
    </label>

    <div class="pick-list">
      <div class="field-head">
        <span>计划图片</span>
        <strong>{{ draft.images.length }} 张</strong>
      </div>
      <div v-if="images.length === 0" class="empty-state small">暂无可选图片</div>
      <label v-for="image in images" v-else :key="image.sha256" class="check-row">
        <input
          :checked="draft.images.includes(image.sha256)"
          type="checkbox"
          @change="toggleImage(image.sha256)"
        />
        <span class="status-dot" :class="image.status"></span>
        <code>{{ shortSha(image.sha256) }}</code>
        <span class="muted">{{ image.remark || '未填写备注' }}</span>
      </label>
    </div>

    <div class="form-actions">
      <button class="button primary" type="submit">{{ plan ? '保存计划' : '创建计划' }}</button>
      <button class="button secondary" type="button" @click="reset">重置</button>
    </div>
  </form>
</template>

<script setup lang="ts">
import { reactive, watch } from 'vue';
import type { AdminImage, AdminPlan, PlanPayload } from '../api';

const props = defineProps<{
  images: AdminImage[];
  plan?: AdminPlan | null;
}>();

const emit = defineEmits<{
  submit: [payload: PlanPayload, id?: number];
  cancel: [];
}>();

const draft = reactive<PlanPayload>({
  start: '',
  end: '',
  caption: '',
  images: [],
});

function reset() {
  load(props.plan);
}

function load(plan?: AdminPlan | null) {
  draft.start = plan?.start ?? '';
  draft.end = plan?.end ?? '';
  draft.caption = plan?.caption ?? '';
  draft.images = plan?.images.map((image) => image.sha256) ?? [];
}

function toggleImage(sha256: string) {
  if (draft.images.includes(sha256)) {
    draft.images = draft.images.filter((item) => item !== sha256);
    return;
  }

  draft.images = [...draft.images, sha256];
}

function submit() {
  emit(
    'submit',
    {
      start: draft.start,
      end: draft.end,
      caption: draft.caption,
      images: draft.images,
    },
    props.plan?.id,
  );

  if (!props.plan) {
    load(null);
  }
}

function shortSha(sha256: string) {
  return sha256.length > 18 ? `${sha256.slice(0, 12)}...${sha256.slice(-6)}` : sha256;
}

watch(
  () => props.plan,
  (plan) => load(plan),
  { immediate: true },
);
</script>
