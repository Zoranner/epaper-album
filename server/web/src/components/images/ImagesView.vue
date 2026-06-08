<template>
  <section class="module-view">
    <header class="module-toolbar">
      <div>
        <h2>图片库</h2>
        <p>素材按电子相册显示比例管理</p>
      </div>
      <div class="module-actions">
        <BaseInput
          label=""
          placeholder="搜索备注或 sha256"
          type="search"
          :model-value="keyword"
          @update:model-value="keyword = $event"
        />
        <select v-model="statusFilter" aria-label="状态筛选">
          <option value="all">全部状态</option>
          <option value="ready">可显示</option>
          <option value="pending">待处理</option>
          <option value="processing">处理中</option>
          <option value="failed">处理失败</option>
        </select>
        <BaseButton small type="button" variant="secondary" @click="loadImages">查询</BaseButton>
        <BaseButton small type="button" variant="primary" @click="openUpload">上传图片</BaseButton>
      </div>
    </header>

    <p v-if="error" class="form-error">{{ error }}</p>
    <ImageGrid
      v-else
      :images="filteredImages"
      :preview-urls="previewUrls"
      @edit-remark="openRemark"
      @refresh-preview="refreshPreview"
    />

    <ImageUploadDialog
      :open="uploadOpen"
      @close="uploadOpen = false"
      @uploaded="handleUploaded"
    />
    <ImageRemarkDialog
      :image="remarkImage"
      :open="Boolean(remarkImage)"
      @close="remarkImage = null"
      @saved="handleRemarkSaved"
    />
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import {
  getImageBlob,
  listImages as listImagesRequest,
  type AdminImage,
  type ImageStatus,
} from '../../api';
import BaseButton from '../base/BaseButton.vue';
import BaseInput from '../base/BaseInput.vue';
import ImageGrid from './ImageGrid.vue';
import ImageRemarkDialog from './ImageRemarkDialog.vue';
import ImageUploadDialog from './ImageUploadDialog.vue';
import { useAuthStore } from '../../composables/useAuthStore';

const auth = useAuthStore();
const images = ref<AdminImage[]>([]);
const keyword = ref('');
const statusFilter = ref<ImageStatus | 'all'>('all');
const previewUrls = ref<Record<string, string>>({});
const uploadOpen = ref(false);
const remarkImage = ref<AdminImage | null>(null);
const error = ref('');

const filteredImages = computed(() => {
  const term = keyword.value.trim().toLowerCase();
  return images.value.filter((image) => {
    const matchStatus = statusFilter.value === 'all' || image.status === statusFilter.value;
    const matchTerm =
      !term ||
      image.sha256.toLowerCase().includes(term) ||
      image.remark.toLowerCase().includes(term);
    return matchStatus && matchTerm;
  });
});

async function loadImages() {
  if (!auth.token.value) {
    return;
  }

  error.value = '';
  try {
    images.value = await listImagesRequest(auth.token.value, keyword.value);
    await loadReadyPreviews();
  } catch (loadError) {
    error.value = loadError instanceof Error ? loadError.message : '图片加载失败';
  }
}

function openUpload() {
  uploadOpen.value = true;
}

function openRemark(image: AdminImage) {
  remarkImage.value = image;
}

async function handleUploaded(image: AdminImage) {
  uploadOpen.value = false;
  upsertImage(image);
  await loadImages();
}

async function handleRemarkSaved(image: AdminImage) {
  remarkImage.value = null;
  upsertImage(image);
}

function upsertImage(image: AdminImage) {
  const index = images.value.findIndex((item) => item.sha256 === image.sha256);
  if (index === -1) {
    images.value = [image, ...images.value];
    return;
  }
  images.value.splice(index, 1, image);
}

async function loadReadyPreviews() {
  const readySha = new Set(images.value.filter((image) => image.status === 'ready').map((image) => image.sha256));
  for (const sha256 of Object.keys(previewUrls.value)) {
    if (!readySha.has(sha256)) {
      revokePreview(sha256);
    }
  }
  for (const sha256 of readySha) {
    if (!previewUrls.value[sha256]) {
      await refreshPreview(sha256, false);
    }
  }
}

async function refreshPreview(sha256: string, showError = true) {
  if (!auth.token.value) {
    return;
  }

  try {
    revokePreview(sha256);
    const blob = await getImageBlob(auth.token.value, sha256);
    previewUrls.value = {
      ...previewUrls.value,
      [sha256]: URL.createObjectURL(blob),
    };
  } catch (previewError) {
    if (showError) {
      error.value = previewError instanceof Error ? previewError.message : '预览加载失败';
    }
  }
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

watch(statusFilter, () => {
  error.value = '';
});

onMounted(() => {
  void loadImages();
});

onBeforeUnmount(clearPreviews);
</script>
