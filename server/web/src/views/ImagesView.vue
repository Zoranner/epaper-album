<template>
  <section class="module-view">
    <header class="module-toolbar">
      <div>
        <h2>相册管理</h2>
        <p>素材按电子相册显示比例管理</p>
      </div>
      <div class="module-actions">
        <Input
          class="module-search"
          label=""
          placeholder="搜索备注或 sha256"
          small
          type="search"
          :model-value="keyword"
          @update:model-value="keyword = $event"
        />
        <Select v-model="statusFilter" small :options="statusOptions" />
        <Button icon="search" small type="button" variant="secondary" @click="loadImages">
          查询
        </Button>
        <Button class="desktop-action" icon="upload" small type="button" variant="primary" @click="openUpload">
          上传图片
        </Button>
      </div>
    </header>

    <p v-if="error" class="form-error">{{ error }}</p>
    <ImageGrid
      v-else
      :images="filteredImages"
      :preview-urls="previewUrls"
      @edit-remark="openRemark"
      @refresh-preview="refreshPreview"
      @redither-image="handleRedither"
      @delete-image="openDelete"
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
    <Dialog
      :open="Boolean(deleteTarget)"
      title="删除图片"
      description="删除后引用该图片的计划会保留，并显示为未选图片。"
      @close="closeDelete"
    >
      <div v-if="deleteTarget" class="dialog-form">
        <code class="dialog-sha">{{ deleteTarget.sha256 }}</code>
        <DialogActions>
          <Button type="button" variant="secondary" @click="closeDelete">取消</Button>
          <Button :loading="deleting" type="button" variant="danger" @click="confirmDelete">
            删除
          </Button>
        </DialogActions>
      </div>
    </Dialog>
    <Button class="floating-action" icon="upload" type="button" variant="primary" @click="openUpload">
      上传图片
    </Button>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import {
  deleteImage,
  getImageBlob,
  listImages as listImagesRequest,
  reditherImage,
  errorMessage,
  type AdminImage,
  type ImageStatus,
} from '../api';
import { Button, Dialog, DialogActions, Input, Select, type SelectOption } from '../components';
import ImageGrid from '../components/images/ImageGrid.vue';
import ImageRemarkDialog from '../components/images/ImageRemarkDialog.vue';
import ImageUploadDialog from '../components/images/ImageUploadDialog.vue';
import { useAuthStore } from '../composables/useAuthStore';

const auth = useAuthStore();
const images = ref<AdminImage[]>([]);
const keyword = ref('');
const statusFilter = ref<ImageStatus | 'all'>('all');
const previewUrls = ref<Record<string, string>>({});
const uploadOpen = ref(false);
const remarkImage = ref<AdminImage | null>(null);
const deleteTarget = ref<AdminImage | null>(null);
const deleting = ref(false);
const error = ref('');
const statusOptions: SelectOption[] = [
  { label: '全部状态', value: 'all' },
  { label: '可显示', value: 'ready' },
  { label: '待处理', value: 'pending' },
  { label: '处理中', value: 'processing' },
  { label: '处理失败', value: 'failed' },
];

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
    error.value = errorMessage(loadError, '图片加载失败');
  }
}

function openUpload() {
  uploadOpen.value = true;
}

function openRemark(image: AdminImage) {
  remarkImage.value = image;
}

function openDelete(image: AdminImage) {
  deleteTarget.value = image;
}

function closeDelete() {
  if (!deleting.value) {
    deleteTarget.value = null;
  }
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

async function handleRedither(image: AdminImage) {
  if (!auth.token.value) {
    return;
  }

  error.value = '';
  try {
    const updated = await reditherImage(auth.token.value, image.sha256);
    revokePreview(image.sha256);
    upsertImage(updated);
  } catch (reditherError) {
    error.value = errorMessage(reditherError, '图片重新抖动失败');
  }
}

function upsertImage(image: AdminImage) {
  const index = images.value.findIndex((item) => item.sha256 === image.sha256);
  if (index === -1) {
    images.value = [image, ...images.value];
    return;
  }
  images.value.splice(index, 1, image);
}

async function confirmDelete() {
  if (!auth.token.value || !deleteTarget.value) {
    return;
  }

  deleting.value = true;
  error.value = '';
  const sha256 = deleteTarget.value.sha256;
  try {
    await deleteImage(auth.token.value, sha256);
    images.value = images.value.filter((image) => image.sha256 !== sha256);
    revokePreview(sha256);
    deleteTarget.value = null;
  } catch (deleteError) {
    error.value = errorMessage(deleteError, '图片删除失败');
  } finally {
    deleting.value = false;
  }
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
      error.value = errorMessage(previewError, '预览加载失败');
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
