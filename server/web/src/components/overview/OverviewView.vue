<template>
  <section class="module-view">
    <header class="module-toolbar">
      <div>
        <h2>概览</h2>
        <p>当前资源和排期状态</p>
      </div>
      <BaseButton small :loading="loading" type="button" variant="secondary" @click="loadOverview">
        刷新
      </BaseButton>
    </header>

    <div class="summary-table">
      <div class="summary-section">
        <h3>资源状态</h3>
        <dl>
          <div>
            <dt>图片总数</dt>
            <dd>{{ images.length }}</dd>
          </div>
          <div>
            <dt>可显示</dt>
            <dd>{{ readyCount }}</dd>
          </div>
          <div>
            <dt>处理中</dt>
            <dd>{{ processingCount }}</dd>
          </div>
          <div>
            <dt>失败</dt>
            <dd>{{ failedCount }}</dd>
          </div>
        </dl>
      </div>

      <div class="summary-section">
        <h3>排期状态</h3>
        <dl>
          <div>
            <dt>计划数量</dt>
            <dd>{{ plans.length }}</dd>
          </div>
          <div>
            <dt>已配图片计划</dt>
            <dd>{{ configuredPlanCount }}</dd>
          </div>
          <div>
            <dt>查询范围</dt>
            <dd>3 天</dd>
          </div>
        </dl>
      </div>
    </div>

    <section class="recent-plans">
      <h3>最近计划</h3>
      <p v-if="error" class="form-error">{{ error }}</p>
      <BaseEmpty v-else-if="plans.length === 0" small>暂无计划</BaseEmpty>
      <div v-else class="recent-plan-list">
        <div v-for="plan in plans" :key="plan.date" class="recent-plan-row">
          <span>{{ plan.caption }}</span>
          <code>{{ plan.date }}</code>
          <strong>{{ shortSha(plan.image_sha256) }}</strong>
        </div>
      </div>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { listImages, listPlans, type AdminImage, type AdminPlan } from '../../api';
import BaseButton from '../base/BaseButton.vue';
import BaseEmpty from '../base/BaseEmpty.vue';
import { useAuthStore } from '../../composables/useAuthStore';

const auth = useAuthStore();
const images = ref<AdminImage[]>([]);
const plans = ref<AdminPlan[]>([]);
const loading = ref(false);
const error = ref('');

const readyCount = computed(() => images.value.filter((image) => image.status === 'ready').length);
const processingCount = computed(
  () => images.value.filter((image) => image.status === 'pending' || image.status === 'processing').length,
);
const failedCount = computed(() => images.value.filter((image) => image.status === 'failed').length);
const configuredPlanCount = computed(() => plans.value.reduce((sum, plan) => sum + (plan.image_sha256 ? 1 : 0), 0));

function shortSha(sha256: string) {
  return sha256.length > 16 ? `${sha256.slice(0, 8)}...${sha256.slice(-6)}` : sha256 || '未配图';
}

async function loadOverview() {
  if (!auth.token.value) {
    return;
  }

  loading.value = true;
  error.value = '';
  try {
    const [nextImages, nextPlans] = await Promise.all([
      listImages(auth.token.value),
      listPlans(auth.token.value, 3),
    ]);
    images.value = nextImages;
    plans.value = nextPlans;
  } catch (loadError) {
    error.value = loadError instanceof Error ? loadError.message : '概览加载失败';
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  void loadOverview();
});
</script>
