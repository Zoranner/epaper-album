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
        <Select v-model="draftType" label="类型" required :options="typeOptions" />
      </div>

      <PlanImagePicker
        v-if="draftType === 'fixed'"
        :images="images"
        :preview-urls="previewUrls"
        :selected="selectedImage"
        @select="selectImage"
      />
      <Input
        v-else
        label="随机标签"
        :maxlength="120"
        placeholder="例如：家庭 旅行"
        :model-value="tagInput"
        @update:model-value="tagInput = $event"
      />

      <p v-if="error" class="form-error">{{ error }}</p>
      <DialogActions>
        <template #meta>{{ metaText }}</template>
        <Button type="button" variant="secondary" @click="$emit('close')">取消</Button>
        <Button :loading="saving" type="submit" variant="primary">保存</Button>
      </DialogActions>
    </form>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import {
  createPlan,
  errorMessage,
  parseTagInput,
  updatePlan,
  type AdminImage,
  type Plan,
  type PlanType,
} from '../../api';
import Button from '../base/Button.vue';
import DatePicker from '../input/DatePicker.vue';
import Input from '../input/Input.vue';
import Select, { type SelectOption } from '../input/Select.vue';
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
  type: 'fixed',
  image: '',
  tags: [],
});
const typeOptions: SelectOption[] = [
  { label: '固定', value: 'fixed' },
  { label: '随机', value: 'random' },
];
const draftType = ref<PlanType>('fixed');
const tagInput = ref('');

const metaText = computed(() => {
  if (draftType.value === 'random') {
    const count = parseTagInput(tagInput.value).length;
    return count > 0 ? `已选 ${count} 个标签` : '未选标签';
  }
  return selectedImage.value ? '已选 1 张' : '未选图片';
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
    const payload: Plan = {
      date: draft.date,
      caption: draft.caption,
      type: draftType.value,
      image: draftType.value === 'fixed' ? selectedImage.value : '',
      tags: draftType.value === 'random' ? parseTagInput(tagInput.value) : [],
    };
    if (props.plan) {
      await updatePlan(auth.token.value, props.plan.date, payload);
    } else {
      await createPlan(auth.token.value, payload);
    }
    emit('saved');
  } catch (saveError) {
    error.value = errorMessage(saveError, '计划保存失败');
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
  draftType.value = plan?.type ?? 'fixed';
  selectedImage.value = plan?.image ?? '';
  draft.image = selectedImage.value;
  tagInput.value = plan?.tags?.join(' ') ?? '';
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
