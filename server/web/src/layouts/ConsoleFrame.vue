<template>
  <div class="console-frame">
    <header class="console-header">
      <div class="console-brand">
        <h1>电子相册管理台</h1>
      </div>

      <div class="console-account">
        <span>管理员</span>
        <Button small type="button" variant="ghost" @click="$emit('logout')">退出</Button>
      </div>
    </header>

    <main class="console-body">
      <slot></slot>
    </main>

    <nav class="console-nav" aria-label="管理模块">
      <button
        v-for="section in sections"
        :key="section.key"
        class="console-nav__item"
        :class="{ active: activeSection === section.key }"
        type="button"
        @click="$emit('changeSection', section.key)"
      >
        <Icon :name="section.icon" />
        {{ section.label }}
      </button>
    </nav>
  </div>
</template>

<script setup lang="ts">
import { Button, Icon, type IconName } from '../components';

export type ConsoleSection = 'images' | 'plans';

defineProps<{
  activeSection: ConsoleSection;
}>();

defineEmits<{
  changeSection: [section: ConsoleSection];
  logout: [];
}>();

const sections: Array<{ key: ConsoleSection; label: string; icon: IconName }> = [
  { key: 'images', label: '相册', icon: 'images' },
  { key: 'plans', label: '计划', icon: 'calendar' },
];
</script>
