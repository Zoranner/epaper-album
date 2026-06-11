<template>
  <Dialog :open="open" :title="plan ? '编辑计划' : '新增计划'" @close="$emit('close')">
    <form class="plan-dialog" @submit.prevent="submit">
      <div class="plan-dialog__fields">
        <Input
          label="标题"
          :maxlength="80"
          required
          :model-value="draft.caption"
          @update:model-value="draft.caption = $event"
        />
        <DatePicker
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
      <DialogActions>
        <template #meta>{{ selectedImage ? '已选 1 张' : '未选图片' }}</template>
        <Button type="button" variant="secondary" @click="$emit('close')">取消</Button>
        <Button :loading="saving" type="submit" variant="primary">保存</Button>
      </DialogActions>
    </form>
  </Dialog>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import { createPlan, updatePlan, type AdminImage, type Plan } from '../../api';
import Button from '../base/Button.vue';
import DatePicker from '../input/DatePicker.vue';
import Input from '../input/Input.vue';
import Dialog from '../overlay/Dialog.vue';
import DialogActions from '../overlay/DialogActions.vue';
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
const draft = reactive<Plan>({
  date: '',
  caption: '',
  image: '',
});

async function submit() {
  if (!auth.token.value) {
    return;
  }

  saving.value = true;
  error.value = '';
  try {
    if (!draft.date) {
      throw new Error('请选择日期');
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
  draft.image = selectedImage.value;
}

function loadDraft(plan: PlanView | null) {
  draft.date = plan?.date ?? todayDate();
  draft.caption = plan?.caption ?? '';
  selectedImage.value = plan?.image ?? '';
  draft.image = selectedImage.value;
  error.value = '';
}

function todayDate() {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
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
