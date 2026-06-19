<template>
  <section class="module-view">
    <ModuleToolbar title="计划管理" description="维护设备显示计划记录">
      <Button icon="plus" small type="button" variant="primary" @click="openCreate">
        新增计划
      </Button>
    </ModuleToolbar>

    <p v-if="error" class="form-error">{{ error }}</p>
    <PlanCalendar
      v-else
      v-model:month="currentMonth"
      :plans="sortedPlans"
      :preview-urls="previewUrls"
      @create-plan="openCreateForDate"
      @delete-plan="openDelete"
      @edit-plan="openEdit"
    />

    <PlanEditorDialog
      :images="images"
      :initial-date="creatingDate"
      :open="editorOpen"
      :plan="editingPlan"
      :preview-urls="previewUrls"
      @close="closeEditor"
      @saved="handleSaved"
    />

    <Dialog :open="Boolean(deletingPlan)" title="删除计划" @close="deletingPlan = null">
      <div v-if="deletingPlan" class="confirm-body">
        <p>确认删除计划“{{ deletingPlan.caption }}”？</p>
        <DialogActions>
          <Button type="button" variant="secondary" @click="deletingPlan = null">取消</Button>
          <Button :loading="deleting" type="button" variant="danger" @click="deleteSelectedPlan">
            删除
          </Button>
        </DialogActions>
      </div>
    </Dialog>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import {
  deletePlan,
  getImageBlob,
  listImages,
  listPlansByRange,
  errorMessage,
  type AdminImage,
  type Plan,
} from '../api';
import { Button, Dialog, DialogActions, ModuleToolbar } from '../components';
import PlanEditorDialog from '../components/plans/PlanEditorDialog.vue';
import PlanCalendar from '../components/plans/PlanCalendar.vue';
import type { PlanView } from '../components/plans/types';
import { useAuthStore } from '../composables/useAuthStore';

const auth = useAuthStore();
const currentMonth = ref(monthString(new Date()));
const plans = ref<PlanView[]>([]);
const images = ref<AdminImage[]>([]);
const previewUrls = ref<Record<string, string>>({});
const editorOpen = ref(false);
const editingPlan = ref<PlanView | null>(null);
const deletingPlan = ref<PlanView | null>(null);
const deleting = ref(false);
const error = ref('');
const creatingDate = ref('');
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
    const { start, end } = monthRange(currentMonth.value);
    const [nextPlans, nextImages] = await Promise.all([
      listPlansByRange(auth.token.value, start, end),
      listImages(auth.token.value),
    ]);
    plans.value = withPlanImages(nextPlans, nextImages);
    images.value = nextImages;
    await loadPreviews(plans.value, nextImages);
  } catch (loadError) {
    error.value = errorMessage(loadError, '计划加载失败');
  }
}

function openCreate() {
  creatingDate.value = '';
  editingPlan.value = null;
  editorOpen.value = true;
}

function openCreateForDate(date: string) {
  creatingDate.value = date;
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
  creatingDate.value = '';
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
    error.value = errorMessage(deleteError, '计划删除失败');
  } finally {
    deleting.value = false;
  }
}

async function loadPreviews(nextPlans: PlanView[], nextImages: AdminImage[]) {
  const readySha = new Set<string>();
  for (const plan of nextPlans) {
    if (plan.imageRecord?.status === 'ready') {
      readySha.add(plan.imageRecord.sha256);
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

function withPlanImages(nextPlans: Plan[], nextImages: AdminImage[]): PlanView[] {
  const imageBySha = new Map(nextImages.map((image) => [image.sha256, image]));
  return nextPlans.map((plan) => ({
    ...plan,
    type: plan.type ?? 'fixed',
    tags: plan.tags ?? [],
    imageRecord: imageBySha.get(plan.image) ?? null,
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

watch(currentMonth, () => {
  void loadPlans();
});

function parseMonth(month: string) {
  const [year, monthNumber] = month.split('-').map(Number);
  return new Date(year, monthNumber - 1, 1);
}

function monthString(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`;
}

function dateString(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

function monthRange(month: string) {
  const startDate = parseMonth(month);
  const endDate = new Date(startDate);
  endDate.setMonth(startDate.getMonth() + 1);
  endDate.setDate(0);
  return {
    start: dateString(startDate),
    end: dateString(endDate),
  };
}
</script>
