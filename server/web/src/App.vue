<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="header-inner">
        <div>
          <h1>电子相册管理台</h1>
          <p>图片处理、计划维护和设备显示资源管理</p>
        </div>
        <div v-if="isLoggedIn" class="header-actions">
          <span class="token-badge">管理员</span>
          <button class="button secondary" type="button" @click="logout">退出</button>
        </div>
      </div>
    </header>

    <main v-if="!isLoggedIn" class="login-view">
      <form class="login-panel" @submit.prevent="login">
        <div class="section-title">
          <div>
            <h2>管理员登录</h2>
            <p>登录后使用管理员 token 调用管理接口</p>
          </div>
        </div>

        <label>
          <span>账号</span>
          <input v-model.trim="loginForm.username" autocomplete="username" required />
        </label>
        <label>
          <span>密码</span>
          <input
            v-model="loginForm.password"
            autocomplete="current-password"
            required
            type="password"
          />
        </label>

        <button class="button primary full" :disabled="loggingIn" type="submit">
          {{ loggingIn ? '登录中' : '登录' }}
        </button>
      </form>
    </main>

    <main v-else class="workspace">
      <section class="panel overview-panel">
        <div class="section-title">
          <div>
            <h2>工作台</h2>
            <p>接口使用 <code>Authorization: Bearer &lt;token&gt;</code></p>
          </div>
          <button class="button secondary" :disabled="loading" type="button" @click="loadAll">刷新</button>
        </div>

        <div class="stats-row">
          <div>
            <span>计划</span>
            <strong>{{ plans.length }}</strong>
          </div>
          <div>
            <span>图片</span>
            <strong>{{ images.length }}</strong>
          </div>
          <div>
            <span>可预览</span>
            <strong>{{ readyImageCount }}</strong>
          </div>
        </div>
      </section>

      <section class="panel">
        <div class="section-title">
          <div>
            <h2>图片管理</h2>
            <p>上传原图、搜索和维护备注</p>
          </div>
          <button class="button secondary" :disabled="loadingImages" type="button" @click="refreshImages">
            刷新图片
          </button>
        </div>

        <div class="toolbar">
          <label>
            <span>备注搜索</span>
            <input v-model.trim="imageKeyword" placeholder="输入关键词" @keyup.enter="refreshImages" />
          </label>
          <button class="button secondary" :disabled="loadingImages" type="button" @click="refreshImages">
            搜索
          </button>
        </div>

        <form class="upload-box" @submit.prevent="uploadSelected">
          <div class="form-grid">
            <label>
              <span>原始图片</span>
              <input ref="fileInput" accept="image/*" required type="file" @change="selectFile" />
            </label>
            <label>
              <span>备注</span>
              <input v-model.trim="uploadRemark" maxlength="120" placeholder="例如：海边晚风" />
            </label>
          </div>
          <button class="button primary" :disabled="!selectedFile || uploading" type="submit">
            {{ uploading ? '上传中' : '上传图片' }}
          </button>
        </form>

        <div v-if="images.length === 0" class="empty-state">暂无图片</div>
        <div v-else class="image-list">
          <article v-for="image in images" :key="image.sha256" class="image-card">
            <div class="preview-box">
              <img v-if="previewUrls[image.sha256]" :src="previewUrls[image.sha256]" :alt="image.sha256" />
              <span v-else>{{ imageStatusText(image.status) }}</span>
            </div>

            <div class="image-info">
              <div class="meta-line">
                <code>{{ image.sha256 }}</code>
                <span class="status-pill" :class="image.status">{{ imageStatusText(image.status) }}</span>
              </div>
              <p v-if="image.status === 'failed'" class="hint">处理失败，可重新上传同一图片触发重试。</p>
              <label>
                <span>备注</span>
                <input v-model="remarkDrafts[image.sha256]" maxlength="120" placeholder="未填写备注" />
              </label>
            </div>

            <div class="row-actions">
              <button
                class="button secondary small"
                :disabled="savingRemark[image.sha256]"
                type="button"
                @click="saveRemark(image)"
              >
                保存备注
              </button>
              <button
                v-if="image.status === 'ready'"
                class="button ghost small"
                type="button"
                @click="refreshPreview(image.sha256)"
              >
                刷新预览
              </button>
            </div>
          </article>
        </div>
      </section>

      <section class="panel">
        <div class="section-title">
          <div>
            <h2>计划管理</h2>
            <p>从今天开始读取指定天数内的计划</p>
          </div>
          <button class="button secondary" :disabled="loadingPlans" type="button" @click="refreshPlans">
            刷新计划
          </button>
        </div>

        <div class="toolbar">
          <label class="days-field">
            <span>天数</span>
            <input v-model.number="planDays" max="7" min="1" type="number" @change="refreshPlans" />
          </label>
          <button class="button primary" type="button" @click="startCreatePlan">新增计划</button>
        </div>

        <PlanEditor
          v-if="showPlanEditor"
          :images="images"
          :plan="editingPlan"
          @cancel="cancelPlanEdit"
          @submit="savePlan"
        />

        <div v-if="plans.length === 0" class="empty-state">暂无计划</div>
        <div v-else class="plan-list">
          <article v-for="plan in plans" :key="plan.id" class="plan-card">
            <div class="plan-head">
              <div>
                <h3>{{ plan.caption }}</h3>
                <p>{{ plan.start }} 至 {{ plan.end }}</p>
              </div>
              <span>{{ plan.images.length }} 张</span>
            </div>

            <div v-if="plan.images.length === 0" class="empty-state small">未选择图片</div>
            <div v-else class="plan-images">
              <div v-for="image in plan.images" :key="image.sha256" class="plan-image">
                <img v-if="previewUrls[image.sha256]" :src="previewUrls[image.sha256]" :alt="image.sha256" />
                <span v-else class="image-placeholder">{{ imageStatusText(image.status) }}</span>
                <div>
                  <code>{{ shortSha(image.sha256) }}</code>
                  <span class="muted">{{ image.remark || '未填写备注' }}</span>
                </div>
                <span class="status-pill" :class="image.status">{{ imageStatusText(image.status) }}</span>
              </div>
            </div>

            <div class="card-actions">
              <button class="button secondary small" type="button" @click="editPlan(plan)">编辑</button>
              <button class="button danger small" type="button" @click="deletePlan(plan.id)">删除</button>
            </div>
          </article>
        </div>
      </section>

      <p v-if="status" class="status-line" :class="{ error: statusType === 'error' }">{{ status }}</p>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue';
import { albumApi, type AdminImage, type AdminPlan, type PlanPayload } from './api';
import PlanEditor from './components/PlanEditor.vue';

const token = ref(localStorage.getItem('epaper-album-admin-token') || '');
const loginForm = reactive({
  username: '',
  password: '',
});
const images = ref<AdminImage[]>([]);
const plans = ref<AdminPlan[]>([]);
const previewUrls = ref<Record<string, string>>({});
const previewErrors = ref<Record<string, string>>({});
const remarkDrafts = reactive<Record<string, string>>({});
const savingRemark = reactive<Record<string, boolean>>({});
const imageKeyword = ref('');
const uploadRemark = ref('');
const selectedFile = ref<File | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const planDays = ref(3);
const editingPlan = ref<AdminPlan | null>(null);
const showPlanEditor = ref(false);
const loggingIn = ref(false);
const loading = ref(false);
const loadingImages = ref(false);
const loadingPlans = ref(false);
const uploading = ref(false);
const status = ref('');
const statusType = ref<'info' | 'error'>('info');

const isLoggedIn = computed(() => token.value.length > 0);
const readyImageCount = computed(() => images.value.filter((image) => image.status === 'ready').length);

function setStatus(message: string, type: 'info' | 'error' = 'info') {
  status.value = message;
  statusType.value = type;
}

async function login() {
  loggingIn.value = true;
  try {
    const result = await albumApi.login(loginForm.username, loginForm.password);
    token.value = result.token;
    localStorage.setItem('epaper-album-admin-token', result.token);
    loginForm.password = '';
    await loadAll();
    setStatus('已登录');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '登录失败', 'error');
  } finally {
    loggingIn.value = false;
  }
}

function logout() {
  token.value = '';
  localStorage.removeItem('epaper-album-admin-token');
  images.value = [];
  plans.value = [];
  clearPreviews();
  setStatus('');
}

async function loadAll() {
  if (!token.value) {
    return;
  }

  loading.value = true;
  try {
    await Promise.all([loadImages(false), loadPlans(false)]);
    setStatus('数据已刷新');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '数据加载失败', 'error');
  } finally {
    loading.value = false;
  }
}

async function loadImages(reportError = true) {
  if (!token.value) {
    return;
  }

  loadingImages.value = true;
  try {
    images.value = await albumApi.listImages(token.value, imageKeyword.value);
    syncRemarkDrafts();
    await loadReadyPreviews();
  } catch (error) {
    if (reportError) {
      setStatus(error instanceof Error ? error.message : '图片加载失败', 'error');
    }
    throw error;
  } finally {
    loadingImages.value = false;
  }
}

async function loadPlans(reportError = true) {
  if (!token.value) {
    return;
  }

  loadingPlans.value = true;
  try {
    planDays.value = clampDays(planDays.value);
    plans.value = await albumApi.listPlans(token.value, planDays.value);
    await loadReadyPreviews();
  } catch (error) {
    if (reportError) {
      setStatus(error instanceof Error ? error.message : '计划加载失败', 'error');
    }
    throw error;
  } finally {
    loadingPlans.value = false;
  }
}

function refreshImages() {
  void loadImages();
}

function refreshPlans() {
  void loadPlans();
}

function selectFile(event: Event) {
  const input = event.target as HTMLInputElement;
  selectedFile.value = input.files?.[0] ?? null;
}

async function uploadSelected() {
  if (!selectedFile.value || !token.value) {
    return;
  }

  uploading.value = true;
  try {
    const image = await albumApi.uploadImage(token.value, selectedFile.value, uploadRemark.value);
    upsertImage(image);
    syncRemarkDrafts();
    selectedFile.value = null;
    uploadRemark.value = '';
    if (fileInput.value) {
      fileInput.value.value = '';
    }
    await Promise.all([loadImages(), loadPlans()]);
    setStatus('图片已上传');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '图片上传失败', 'error');
  } finally {
    uploading.value = false;
  }
}

async function saveRemark(image: AdminImage) {
  if (!token.value) {
    return;
  }

  savingRemark[image.sha256] = true;
  try {
    const updated = await albumApi.updateImageRemark(
      token.value,
      image.sha256,
      remarkDrafts[image.sha256] ?? '',
    );
    upsertImage(updated);
    replacePlanImage(updated);
    setStatus('备注已保存');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '备注保存失败', 'error');
  } finally {
    savingRemark[image.sha256] = false;
  }
}

function startCreatePlan() {
  editingPlan.value = null;
  showPlanEditor.value = true;
}

function editPlan(plan: AdminPlan) {
  editingPlan.value = plan;
  showPlanEditor.value = true;
}

function cancelPlanEdit() {
  editingPlan.value = null;
  showPlanEditor.value = false;
}

async function savePlan(payload: PlanPayload, id?: number) {
  if (!token.value) {
    return;
  }

  try {
    const saved =
      id === undefined
        ? await albumApi.createPlan(token.value, payload)
        : await albumApi.updatePlan(token.value, id, payload);
    upsertPlan(saved);
    await loadReadyPreviews();
    cancelPlanEdit();
    setStatus(id === undefined ? '计划已创建' : '计划已保存');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '计划保存失败', 'error');
  }
}

async function deletePlan(id: number) {
  if (!token.value || !window.confirm(`删除计划 ${id}？`)) {
    return;
  }

  try {
    await albumApi.deletePlan(token.value, id);
    plans.value = plans.value.filter((plan) => plan.id !== id);
    if (editingPlan.value?.id === id) {
      cancelPlanEdit();
    }
    setStatus('计划已删除');
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '计划删除失败', 'error');
  }
}

function syncRemarkDrafts() {
  const active = new Set(images.value.map((image) => image.sha256));
  for (const key of Object.keys(remarkDrafts)) {
    if (!active.has(key)) {
      delete remarkDrafts[key];
    }
  }
  for (const image of images.value) {
    remarkDrafts[image.sha256] = image.remark;
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

function upsertPlan(plan: AdminPlan) {
  const index = plans.value.findIndex((item) => item.id === plan.id);
  if (index === -1) {
    plans.value = [plan, ...plans.value];
    return;
  }

  plans.value.splice(index, 1, plan);
}

function replacePlanImage(image: AdminImage) {
  plans.value = plans.value.map((plan) => ({
    ...plan,
    images: plan.images.map((item) => (item.sha256 === image.sha256 ? image : item)),
  }));
}

async function loadReadyPreviews() {
  if (!token.value) {
    return;
  }

  const ready = new Set<AdminImage>();
  for (const image of images.value) {
    if (image.status === 'ready') {
      ready.add(image);
    }
  }
  for (const plan of plans.value) {
    for (const image of plan.images) {
      if (image.status === 'ready') {
        ready.add(image);
      }
    }
  }

  const activeSha = new Set(Array.from(ready).map((image) => image.sha256));
  for (const sha256 of Object.keys(previewUrls.value)) {
    if (!activeSha.has(sha256)) {
      revokePreview(sha256);
    }
  }

  for (const image of ready) {
    if (!previewUrls.value[image.sha256] && !previewErrors.value[image.sha256]) {
      await refreshPreview(image.sha256, false);
    }
  }
}

async function refreshPreview(sha256: string, notify = true) {
  if (!token.value) {
    return;
  }

  try {
    revokePreview(sha256);
    const url = await albumApi.getImageObjectUrl(token.value, sha256);
    previewUrls.value = {
      ...previewUrls.value,
      [sha256]: url,
    };
    delete previewErrors.value[sha256];
    if (notify) {
      setStatus('预览已刷新');
    }
  } catch (error) {
    previewErrors.value = {
      ...previewErrors.value,
      [sha256]: error instanceof Error ? error.message : '预览加载失败',
    };
    if (notify) {
      setStatus(previewErrors.value[sha256], 'error');
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
  previewErrors.value = {};
}

function imageStatusText(statusValue: AdminImage['status']) {
  if (statusValue === 'ready') {
    return '可预览';
  }
  if (statusValue === 'failed') {
    return '处理失败';
  }
  return '处理中';
}

function shortSha(sha256: string) {
  return sha256.length > 18 ? `${sha256.slice(0, 12)}...${sha256.slice(-6)}` : sha256;
}

function clampDays(days: number) {
  if (!Number.isFinite(days)) {
    return 3;
  }
  return Math.min(7, Math.max(1, Math.trunc(days)));
}

onMounted(() => {
  if (token.value) {
    void loadAll();
  }
});

onBeforeUnmount(clearPreviews);
</script>
