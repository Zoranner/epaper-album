<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="header-inner">
        <div>
          <h1>电子相册管理台</h1>
          <p>Manifest、计划项与图片资源管理</p>
        </div>
        <span class="header-code">epaper-album</span>
      </div>
    </header>

    <main class="workspace">
      <section class="panel info-panel">
        <div class="section-title">
          <div>
            <h2>设备接口</h2>
            <p>设备读取 manifest 并按 sha256 下载图片资源</p>
          </div>
          <span class="badge">secret-key</span>
        </div>

        <dl class="info-grid">
          <div>
            <dt>Manifest</dt>
            <dd><code>GET /api/manifest</code></dd>
          </div>
          <div>
            <dt>图片资源</dt>
            <dd><code>GET /images/:sha256</code></dd>
          </div>
          <div>
            <dt>鉴权说明</dt>
            <dd>设备请求接口时在请求头携带 <code>secret-key</code>，取值使用服务端配置的设备密钥。</dd>
          </div>
        </dl>

        <label class="secret-field">
          <span>管理密钥</span>
          <input v-model.trim="secretKey" type="password" autocomplete="current-password" />
        </label>
      </section>

      <section class="panel">
        <div class="section-title">
          <div>
            <h2>当前 Manifest</h2>
            <p>版本号与计划数量</p>
          </div>
          <button class="button secondary" type="button" :disabled="loading" @click="loadAll">刷新</button>
        </div>

        <div class="stats-row">
          <div>
            <span>Version</span>
            <strong>{{ manifest.version }}</strong>
          </div>
          <div>
            <span>Plans</span>
            <strong>{{ manifest.plans.length }}</strong>
          </div>
          <div>
            <span>Images</span>
            <strong>{{ images.length }}</strong>
          </div>
        </div>

        <pre class="manifest-preview">{{ manifestJson }}</pre>
      </section>

      <PlanEditor
        :editing="editingIndex !== null"
        :plan="editingPlan"
        @cancel="cancelEdit"
        @submit="savePlan"
      />

      <section class="panel">
        <div class="section-title">
          <div>
            <h2>计划项</h2>
            <p>按 start/end 生效，图片按 sha256 顺序轮换</p>
          </div>
          <button class="button primary" type="button" :disabled="saving" @click="saveManifest">
            保存 Manifest
          </button>
        </div>

        <div v-if="manifest.plans.length === 0" class="empty-state">暂无计划项</div>
        <div v-else class="plan-list">
          <article v-for="(plan, index) in manifest.plans" :key="`${plan.start}-${plan.end}-${index}`" class="plan-card">
            <div class="plan-main">
              <div>
                <h3>{{ plan.caption }}</h3>
                <p>{{ plan.start }} 至 {{ plan.end }}</p>
              </div>
              <span>{{ plan.images.length }} 张</span>
            </div>

            <div class="hash-list">
              <code v-for="sha in plan.images" :key="sha">{{ sha }}</code>
            </div>

            <div class="card-actions">
              <button class="button secondary small" type="button" @click="editPlan(index)">编辑</button>
              <button class="button danger small" type="button" @click="removePlan(index)">删除</button>
            </div>
          </article>
        </div>
      </section>

      <section class="panel upload-panel">
        <div class="section-title">
          <div>
            <h2>上传图片</h2>
            <p>上传后返回 sha256，可填入计划项图片列表</p>
          </div>
        </div>

        <div class="upload-row">
          <input ref="fileInput" type="file" accept="image/*" @change="selectFile" />
          <button class="button primary" type="button" :disabled="!selectedFile || uploading" @click="uploadSelected">
            上传
          </button>
        </div>

        <div v-if="uploadedSha" class="result-box">
          <span>返回 sha256</span>
          <code>{{ uploadedSha }}</code>
        </div>
      </section>

      <section class="panel">
        <div class="section-title">
          <div>
            <h2>图片资源</h2>
            <p>服务端已保存的图片资源</p>
          </div>
          <button class="button secondary" type="button" :disabled="loadingImages" @click="loadImages">
            刷新资源
          </button>
        </div>

        <div v-if="images.length === 0" class="empty-state">暂无图片资源</div>
        <div v-else class="image-grid">
          <article v-for="image in images" :key="image.sha256" class="image-card">
            <img :src="imagePreviews[image.sha256] || ''" :alt="image.sha256" loading="lazy" />
            <div>
              <code>{{ image.sha256 }}</code>
              <span v-if="image.size">{{ formatBytes(image.size) }}</span>
            </div>
            <button class="button danger small" type="button" @click="deleteImage(image.sha256)">删除</button>
          </article>
        </div>
      </section>

      <p v-if="status" class="status-line" :class="{ error: statusType === 'error' }">{{ status }}</p>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { albumApi, type AlbumPlan, type ImageResource, type Manifest } from './api';
import PlanEditor from './components/PlanEditor.vue';

const manifest = ref<Manifest>({ version: '0', plans: [] });
const images = ref<ImageResource[]>([]);
const imagePreviews = ref<Record<string, string>>({});
const loading = ref(false);
const loadingImages = ref(false);
const saving = ref(false);
const uploading = ref(false);
const editingIndex = ref<number | null>(null);
const selectedFile = ref<File | null>(null);
const uploadedSha = ref('');
const status = ref('');
const statusType = ref<'info' | 'error'>('info');
const fileInput = ref<HTMLInputElement | null>(null);
const secretKey = ref(localStorage.getItem('epaper-album-secret-key') || 'local-secret-key');

const manifestJson = computed(() => JSON.stringify(manifest.value, null, 2));
const editingPlan = computed(() =>
  editingIndex.value === null ? null : manifest.value.plans[editingIndex.value],
);

function setStatus(message: string, type: 'info' | 'error' = 'info') {
  status.value = message;
  statusType.value = type;
}

async function loadManifest() {
  manifest.value = await albumApi.getManifest(secretKey.value);
}

async function loadImages() {
  loadingImages.value = true;
  try {
    images.value = await albumApi.listImages(secretKey.value);
    await loadImagePreviews();
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '图片资源加载失败', 'error');
  } finally {
    loadingImages.value = false;
  }
}

async function loadAll() {
  loading.value = true;
  try {
    await Promise.all([loadManifest(), loadImages()]);
    setStatus('数据已刷新');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '数据加载失败', 'error');
  } finally {
    loading.value = false;
  }
}

function savePlan(plan: AlbumPlan) {
  if (editingIndex.value === null) {
    manifest.value.plans.push(plan);
  } else {
    manifest.value.plans.splice(editingIndex.value, 1, plan);
    editingIndex.value = null;
  }
}

function editPlan(index: number) {
  editingIndex.value = index;
}

function cancelEdit() {
  editingIndex.value = null;
}

function removePlan(index: number) {
  manifest.value.plans.splice(index, 1);
  if (editingIndex.value === index) {
    editingIndex.value = null;
  }
}

async function saveManifest() {
  saving.value = true;
  try {
    const nextManifest = {
      version: nextVersion(),
      plans: manifest.value.plans,
    };
    manifest.value = await albumApi.updateManifest(nextManifest, secretKey.value);
    setStatus('Manifest 已保存');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : 'Manifest 保存失败', 'error');
  } finally {
    saving.value = false;
  }
}

function selectFile(event: Event) {
  const input = event.target as HTMLInputElement;
  selectedFile.value = input.files?.[0] ?? null;
  uploadedSha.value = '';
}

async function uploadSelected() {
  if (!selectedFile.value) {
    return;
  }

  uploading.value = true;
  try {
    const result = await albumApi.uploadImage(selectedFile.value, secretKey.value);
    uploadedSha.value = result.sha256;
    selectedFile.value = null;
    if (fileInput.value) {
      fileInput.value.value = '';
    }
    await loadImages();
    setStatus('图片已上传');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '图片上传失败', 'error');
  } finally {
    uploading.value = false;
  }
}

async function deleteImage(sha256: string) {
  const confirmed = window.confirm(`删除图片资源 ${sha256}？`);
  if (!confirmed) {
    return;
  }

  try {
    await albumApi.deleteImage(sha256, secretKey.value);
    images.value = images.value.filter((image) => image.sha256 !== sha256);
    revokeImagePreview(sha256);
    setStatus('图片资源已删除');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '图片删除失败', 'error');
  }
}

async function loadImagePreviews() {
  const active = new Set(images.value.map((image) => image.sha256));
  for (const sha256 of Object.keys(imagePreviews.value)) {
    if (!active.has(sha256)) {
      revokeImagePreview(sha256);
    }
  }

  for (const image of images.value) {
    if (imagePreviews.value[image.sha256]) {
      continue;
    }
    imagePreviews.value[image.sha256] = await albumApi.getImageObjectUrl(
      image.sha256,
      secretKey.value,
    );
  }
}

function revokeImagePreview(sha256: string) {
  const url = imagePreviews.value[sha256];
  if (url) {
    URL.revokeObjectURL(url);
  }
  const next = { ...imagePreviews.value };
  delete next[sha256];
  imagePreviews.value = next;
}

function formatBytes(size: number) {
  if (size < 1024) {
    return `${size} B`;
  }

  if (size < 1024 * 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }

  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

function nextVersion() {
  const now = new Date();
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, '0');
  const dd = String(now.getDate()).padStart(2, '0');
  const hh = String(now.getHours()).padStart(2, '0');
  const mi = String(now.getMinutes()).padStart(2, '0');
  const ss = String(now.getSeconds()).padStart(2, '0');
  return `${yyyy}-${mm}-${dd}-${hh}${mi}${ss}`;
}

watch(secretKey, (value) => {
  localStorage.setItem('epaper-album-secret-key', value);
});

onMounted(loadAll);
</script>
