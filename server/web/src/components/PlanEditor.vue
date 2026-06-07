<template>
  <form class="panel form-panel" @submit.prevent="submit">
    <div class="section-title">
      <div>
        <h2>{{ editing ? '编辑计划' : '新增计划' }}</h2>
        <p>维护设备按时间生效的图片计划项</p>
      </div>
      <button v-if="editing" class="button ghost" type="button" @click="$emit('cancel')">取消编辑</button>
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
      <input v-model.trim="draft.caption" required maxlength="80" placeholder="例如：春节相册" />
    </label>

    <label>
      <span>图片 sha256 列表</span>
      <textarea
        v-model="imagesText"
        required
        rows="6"
        spellcheck="false"
        placeholder="每行一个 sha256"
      />
    </label>

    <div class="form-actions">
      <button class="button primary" type="submit">{{ editing ? '保存计划' : '添加计划' }}</button>
      <button class="button secondary" type="button" @click="reset">清空</button>
    </div>
  </form>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import type { AlbumPlan } from '../api';

const props = defineProps<{
  plan?: AlbumPlan | null;
  editing?: boolean;
}>();

const emit = defineEmits<{
  submit: [plan: AlbumPlan];
  cancel: [];
}>();

const emptyPlan = (): AlbumPlan => ({
  start: '',
  end: '',
  caption: '',
  images: [],
});

const draft = reactive<AlbumPlan>(emptyPlan());
const imagesText = ref('');

const normalizedImages = computed(() =>
  imagesText.value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean),
);

function load(plan?: AlbumPlan | null) {
  const next = plan ?? emptyPlan();
  draft.start = next.start;
  draft.end = next.end;
  draft.caption = next.caption;
  draft.images = [...next.images];
  imagesText.value = next.images.join('\n');
}

function reset() {
  load(props.editing ? props.plan : null);
}

function submit() {
  emit('submit', {
    start: draft.start,
    end: draft.end,
    caption: draft.caption,
    images: normalizedImages.value,
  });

  if (!props.editing) {
    load(null);
  }
}

watch(
  () => props.plan,
  (plan) => load(plan),
  { immediate: true },
);
</script>
