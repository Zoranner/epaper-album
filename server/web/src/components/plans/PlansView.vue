<template>
  <section class="module-view">
    <header class="module-toolbar">
      <div>
        <h2>计划管理</h2>
        <p>按日期维护设备显示排期</p>
      </div>
      <div class="module-actions">
        <BaseNumberInput
          label=""
          :max="7"
          :min="1"
          :model-value="days"
          @update:model-value="days = $event"
        />
        <BaseButton small type="button" variant="secondary" @click="loadPlans">查询</BaseButton>
        <BaseButton small type="button" variant="primary" @click="openCreate">新增计划</BaseButton>
      </div>
    </header>

    <p v-if="error" class="form-error">{{ error }}</p>
    <div v-else class="schedule-list">
      <PlanDayGroup
        v-for="group in dayGroups"
        :key="group.date"
        :date="group.date"
        :plans="group.plans"
        :preview-urls="previewUrls"
        @delete-plan="openDelete"
        @edit-plan="openEdit"
      />
    </div>

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
        <div class="dialog-actions">
          <BaseButton type="button" variant="secondary" @click="deletingPlan = null">取消</BaseButton>
          <BaseButton :loading="deleting" type="button" variant="danger" @click="deleteSelectedPlan">
            删除
          </BaseButton>
        </div>
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
import BaseNumberInput from '../base/BaseNumberInput.vue';
import PlanDayGroup from './PlanDayGroup.vue';
import PlanEditorDialog from './PlanEditorDialog.vue';
import { useAuthStore } from '../../composables/useAuthStore';

interface DayGroup {
  date: string;
  plans: AdminPlan[];
}

const auth = useAuthStore();
const days = ref(3);
const plans = ref<AdminPlan[]>([]);
const images = ref<AdminImage[]>([]);
const previewUrls = ref<Record<string, string>>({});
const editorOpen = ref(false);
const editingPlan = ref<AdminPlan | null>(null);
const deletingPlan = ref<AdminPlan | null>(null);
const deleting = ref(false);
const error = ref('');

const dayGroups = computed<DayGroup[]>(() => {
  const dates = nextDates(days.value);
  return dates.map((date) => ({
    date,
    plans: plans.value.filter((plan) => plan.start <= date && plan.end >= date),
  }));
});

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
    plans.value = nextPlans;
    images.value = nextImages;
    await loadPreviews(nextPlans, nextImages);
  } catch (loadError) {
    error.value = loadError instanceof Error ? loadError.message : '计划加载失败';
  }
}

function openCreate() {
  editingPlan.value = null;
  editorOpen.value = true;
}

function openEdit(plan: AdminPlan) {
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

function openDelete(plan: AdminPlan) {
  deletingPlan.value = plan;
}

async function deleteSelectedPlan() {
  if (!auth.token.value || !deletingPlan.value) {
    return;
  }

  deleting.value = true;
  try {
    await deletePlan(auth.token.value, deletingPlan.value.id);
    deletingPlan.value = null;
    await loadPlans();
  } catch (deleteError) {
    error.value = deleteError instanceof Error ? deleteError.message : '计划删除失败';
  } finally {
    deleting.value = false;
  }
}

async function loadPreviews(nextPlans: AdminPlan[], nextImages: AdminImage[]) {
  const readySha = new Set<string>();
  for (const plan of nextPlans) {
    for (const image of plan.images) {
      if (image.status === 'ready') {
        readySha.add(image.sha256);
      }
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

function nextDates(count: number) {
  const safeCount = Math.min(7, Math.max(1, Math.trunc(count || 3)));
  const result: string[] = [];
  const today = new Date();
  for (let index = 0; index < safeCount; index += 1) {
    const date = new Date(today);
    date.setDate(today.getDate() + index);
    result.push(date.toISOString().slice(0, 10));
  }
  return result;
}

onMounted(() => {
  void loadPlans();
});

onBeforeUnmount(clearPreviews);
</script>
