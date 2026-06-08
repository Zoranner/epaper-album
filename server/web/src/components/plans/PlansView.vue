<template>
  <section class="module-view">
    <header class="module-toolbar">
      <div>
        <h2>计划管理</h2>
        <p>维护设备显示计划记录</p>
      </div>
      <div class="module-actions">
        <BaseSelect v-model="daysValue" small :options="dayOptions" />
        <BaseButton small type="button" variant="secondary" @click="loadPlans">
          <BaseIcon name="search" />
          查询
        </BaseButton>
        <BaseButton small type="button" variant="primary" @click="openCreate">
          <BaseIcon name="plus" />
          新增计划
        </BaseButton>
      </div>
    </header>

    <p v-if="error" class="form-error">{{ error }}</p>
    <PlanTable
      v-else
      :plans="sortedPlans"
      :preview-urls="previewUrls"
      @delete-plan="openDelete"
      @edit-plan="openEdit"
    />

    <PlanEditorDialog
      :images="images"
      :open="editorOpen"
      :plan="editingPlan"
      :preview-urls="previewUrls"
      @close="closeEditor"
      @saved="handleSaved"
    />

    <BaseDialog :open="Boolean(deletingPlan)" title="删除计划" @close="deletingPlan = null">
      <div v-if="deletingPlan" class="confirm-body">
        <p>确认删除计划“{{ deletingPlan.caption }}”？</p>
        <BaseDialogActions>
          <BaseButton type="button" variant="secondary" @click="deletingPlan = null">取消</BaseButton>
          <BaseButton :loading="deleting" type="button" variant="danger" @click="deleteSelectedPlan">
            删除
          </BaseButton>
        </BaseDialogActions>
      </div>
    </BaseDialog>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import {
  deletePlan,
  getImageBlob,
  listImages,
  listPlans as listPlansRequest,
  type AdminImage,
  type AdminPlan,
} from '../../api';
import BaseButton from '../base/BaseButton.vue';
import BaseDialog from '../base/BaseDialog.vue';
import BaseDialogActions from '../base/BaseDialogActions.vue';
import BaseIcon from '../base/BaseIcon.vue';
import BaseSelect, { type BaseSelectOption } from '../base/BaseSelect.vue';
import PlanEditorDialog from './PlanEditorDialog.vue';
import PlanTable from './PlanTable.vue';
import type { PlanView } from './types';
import { useAuthStore } from '../../composables/useAuthStore';

const auth = useAuthStore();
const days = ref(3);
const daysValue = computed({
  get: () => String(days.value),
  set: (value: string) => {
    days.value = Number(value);
  },
});
const plans = ref<PlanView[]>([]);
const images = ref<AdminImage[]>([]);
const previewUrls = ref<Record<string, string>>({});
const editorOpen = ref(false);
const editingPlan = ref<PlanView | null>(null);
const deletingPlan = ref<PlanView | null>(null);
const deleting = ref(false);
const error = ref('');
const dayOptions: BaseSelectOption[] = Array.from({ length: 7 }, (_, index) => {
  const value = String(index + 1);
  return { label: `最近 ${value} 天`, value };
});
const sortedPlans = computed(() =>
  [...plans.value].sort((left, right) => {
    const byDate = left.date.localeCompare(right.date);
    return byDate === 0 ? left.caption.localeCompare(right.caption) : byDate;
  }),
);

async function loadPlans() {
  if (!auth.token.value) {
    return;
  }

  error.value = '';
  try {
    const [nextPlans, nextImages] = await Promise.all([
      listPlansRequest(auth.token.value, days.value),
      listImages(auth.token.value),
    ]);
    plans.value = withPlanImages(nextPlans, nextImages);
    images.value = nextImages;
    await loadPreviews(plans.value, nextImages);
  } catch (loadError) {
    error.value = loadError instanceof Error ? loadError.message : '计划加载失败';
  }
}

function openCreate() {
  editingPlan.value = null;
  editorOpen.value = true;
}

function openEdit(plan: PlanView) {
  editingPlan.value = plan;
  editorOpen.value = true;
}

function closeEditor() {
  editorOpen.value = false;
  editingPlan.value = null;
}

async function handleSaved() {
  closeEditor();
  await loadPlans();
}

function openDelete(plan: PlanView) {
  deletingPlan.value = plan;
}

async function deleteSelectedPlan() {
  if (!auth.token.value || !deletingPlan.value) {
    return;
  }

  deleting.value = true;
  try {
    await deletePlan(auth.token.value, deletingPlan.value.date);
    deletingPlan.value = null;
    await loadPlans();
  } catch (deleteError) {
    error.value = deleteError instanceof Error ? deleteError.message : '计划删除失败';
  } finally {
    deleting.value = false;
  }
}

async function loadPreviews(nextPlans: PlanView[], nextImages: AdminImage[]) {
  const readySha = new Set<string>();
  for (const plan of nextPlans) {
    if (plan.image?.status === 'ready') {
      readySha.add(plan.image.sha256);
    }
  }
  for (const image of nextImages) {
    if (image.status === 'ready') {
      readySha.add(image.sha256);
    }
  }

  for (const sha256 of Object.keys(previewUrls.value)) {
    if (!readySha.has(sha256)) {
      revokePreview(sha256);
    }
  }

  for (const sha256 of readySha) {
    if (!previewUrls.value[sha256]) {
      await refreshPreview(sha256);
    }
  }
}

function withPlanImages(nextPlans: AdminPlan[], nextImages: AdminImage[]): PlanView[] {
  const imageBySha = new Map(nextImages.map((image) => [image.sha256, image]));
  return nextPlans.map((plan) => ({
    ...plan,
    image: imageBySha.get(plan.image_sha256) ?? null,
  }));
}

async function refreshPreview(sha256: string) {
  if (!auth.token.value) {
    return;
  }

  const blob = await getImageBlob(auth.token.value, sha256);
  previewUrls.value = {
    ...previewUrls.value,
    [sha256]: URL.createObjectURL(blob),
  };
}

function revokePreview(sha256: string) {
  const url = previewUrls.value[sha256];
  if (url) {
    URL.revokeObjectURL(url);
  }
  const next = { ...previewUrls.value };
  delete next[sha256];
  previewUrls.value = next;
}

function clearPreviews() {
  for (const url of Object.values(previewUrls.value)) {
    URL.revokeObjectURL(url);
  }
  previewUrls.value = {};
}

onMounted(() => {
  void loadPlans();
});

onBeforeUnmount(clearPreviews);
</script>
