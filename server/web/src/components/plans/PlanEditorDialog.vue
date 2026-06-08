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
        <BaseDateInput
          label="日期"
          required
          :model-value="draft.date"
          @update:model-value="draft.date = $event"
        />
      </div>

      <PlanImagePicker
        :images="images"
        :preview-urls="previewUrls"
        :selected="selectedImage"
        @select="selectImage"
      />

      <p v-if="error" class="form-error">{{ error }}</p>
      <BaseDialogActions>
        <template #meta>{{ selectedImage ? '已选 1 张' : '未选图片' }}</template>
        <BaseButton type="button" variant="secondary" @click="$emit('close')">取消</BaseButton>
        <BaseButton :loading="saving" type="submit" variant="primary">保存</BaseButton>
      </BaseDialogActions>
    </form>
  </BaseDialog>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import { createPlan, updatePlan, type AdminImage, type PlanPayload } from '../../api';
import BaseButton from '../base/BaseButton.vue';
import BaseDialog from '../base/BaseDialog.vue';
import BaseDialogActions from '../base/BaseDialogActions.vue';
import BaseDateInput from '../base/BaseDateInput.vue';
import BaseInput from '../base/BaseInput.vue';
import PlanImagePicker from './PlanImagePicker.vue';
import type { PlanView } from './types';
import { useAuthStore } from '../../composables/useAuthStore';

const props = defineProps<{
  open: boolean;
  plan: PlanView | null;
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
  date: '',
  caption: '',
  image_sha256: '',
});

async function submit() {
  if (!auth.token.value) {
    return;
  }

  saving.value = true;
  error.value = '';
  try {
    if (!draft.image_sha256) {
      throw new Error('请选择一张图片');
    }
    if (props.plan) {
      await updatePlan(auth.token.value, props.plan.date, draft);
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

const selectedImage = ref('');

function selectImage(sha256: string) {
  selectedImage.value = selectedImage.value === sha256 ? '' : sha256;
  draft.image_sha256 = selectedImage.value;
}

function loadDraft(plan: PlanView | null) {
  draft.date = plan?.date ?? '';
  draft.caption = plan?.caption ?? '';
  selectedImage.value = plan?.image_sha256 ?? '';
  draft.image_sha256 = selectedImage.value;
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
