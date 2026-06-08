<template>
  <div class="console-frame">
    <header class="console-header">
      <div class="console-brand">
        <h1>电子相册管理台</h1>
      </div>

      <nav class="console-tabs" aria-label="工作区">
        <button
          v-for="section in sections"
          :key="section.key"
          class="console-tab"
          :class="{ active: activeSection === section.key }"
          type="button"
          @click="$emit('changeSection', section.key)"
        >
          {{ section.label }}
        </button>
      </nav>

      <div class="console-account">
        <span>管理员</span>
        <BaseButton small type="button" variant="ghost" @click="$emit('logout')">退出</BaseButton>
      </div>
    </header>

    <main class="console-body">
      <slot></slot>
    </main>
  </div>
</template>

<script setup lang="ts">
import BaseButton from '../components/base/BaseButton.vue';

export type ConsoleSection = 'overview' | 'images' | 'plans';

defineProps<{
  activeSection: ConsoleSection;
}>();

defineEmits<{
  changeSection: [section: ConsoleSection];
  logout: [];
}>();

const sections: Array<{ key: ConsoleSection; label: string }> = [
  { key: 'overview', label: '概览' },
  { key: 'images', label: '图片库' },
  { key: 'plans', label: '计划管理' },
];
</script>
