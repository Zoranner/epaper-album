<template>
  <ConsoleFrame :active-section="activeSection" @change-section="activeSection = $event" @logout="$emit('logout')">
    <OverviewView v-if="activeSection === 'overview'" />
    <ImagesView v-else-if="activeSection === 'images'" />
    <PlansView v-else />
  </ConsoleFrame>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import OverviewView from '../components/overview/OverviewView.vue';
import ImagesView from '../components/images/ImagesView.vue';
import PlansView from '../components/plans/PlansView.vue';
import ConsoleFrame, { type ConsoleSection } from '../layouts/ConsoleFrame.vue';

defineEmits<{
  logout: [];
}>();

const activeSection = ref<ConsoleSection>('overview');
</script>
