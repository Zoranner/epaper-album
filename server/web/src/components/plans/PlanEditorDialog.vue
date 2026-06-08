<template>
  <BaseDialog :open="open" :title="plan ? '编辑计划' : '新增计划'" @close="$emit('close')">
    <form class="plan-dialog" @submit.prevent="submit">
      <div class="plan-dialog__fields">
        <BaseInput
          label="标题"
          :maxlength="80"
          required
          :model-value="draft.caption"
          @update:model-value="draft.caption = $event"
        />
        <BaseInput
          label="开始日期"
          required
          type="date"
          :model-value="draft.start"
          @update:model-value="draft.start = $event"
        />
        <BaseInput
          label="结束日期"
          required
          type="date"
          :model-value="draft.end"
          @update:model-value="draft.end = $event"
        />
      </div>

      <PlanImagePicker
        :images="images"
        :preview-urls="previewUrls"
        :selected="draft.images"
        @toggle="toggleImage"
      />

      <p v-if="error" class="form-error">{{ error }}</p>
      <div class="dialog-actions">
        <span>已选 {{ draft.images.length }} 张</span>
        <BaseButton type="button" variant="secondary" @click="$emit('close')">取消</BaseButton>
        <BaseButton :loading="saving" type="submit" variant="primary">保存</BaseButton>
      </div>
    </form>
  </BaseDialog>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import { createPlan, updatePlan, type AdminImage, type AdminPlan, type PlanPayload } from '../../api';
import BaseButton from '../base/BaseButton.vue';
import BaseDialog from '../base/BaseDialog.vue';
import BaseInput from '../base/BaseInput.vue';
import PlanImagePicker from './PlanImagePicker.vue';
import { useAuthStore } from '../../composables/useAuthStore';

const props = defineProps<{
  open: boolean;
  plan: AdminPlan | null;
  images: AdminImage[];
  previewUrls: Record<string, string>;
}>();

const emit = defineEmits<{
  close: [];
  saved: [];
}>();

const auth = useAuthStore();
const saving = ref(false);
const error = ref('');
const draft = reactive<PlanPayload>({
  start: '',
  end: '',
  caption: '',
  images: [],
});

async function submit() {
  if (!auth.token.value) {
    return;
  }

  saving.value = true;
  error.value = '';
  try {
    if (props.plan) {
      await updatePlan(auth.token.value, props.plan.id, draft);
    } else {
      await createPlan(auth.token.value, draft);
    }
    emit('saved');
  } catch (saveError) {
    error.value = saveError instanceof Error ? saveError.message : '计划保存失败';
  } finally {
    saving.value = false;
  }
}

function toggleImage(sha256: string) {
  if (draft.images.includes(sha256)) {
    draft.images = draft.images.filter((item) => item !== sha256);
    return;
  }
  draft.images = [...draft.images, sha256];
}

function loadDraft(plan: AdminPlan | null) {
  draft.start = plan?.start ?? '';
  draft.end = plan?.end ?? '';
  draft.caption = plan?.caption ?? '';
  draft.images = plan?.images.map((image) => image.sha256) ?? [];
  error.value = '';
}

watch(
  () => props.plan,
  (plan) => loadDraft(plan),
  { immediate: true },
);

watch(
  () => props.open,
  (open) => {
    if (open) {
      loadDraft(props.plan);
    }
  },
);
</script>
