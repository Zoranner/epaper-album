<template>
  <ConsoleFrame :active-section="activeSection" @change-section="activeSection = $event" @logout="$emit('logout')">
    <ImagesView v-if="activeSection === 'images'" />
    <PlansView v-else />
  </ConsoleFrame>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import ConsoleFrame, { type ConsoleSection } from '../layouts/ConsoleFrame.vue';
import ImagesView from '../views/ImagesView.vue';
import PlansView from '../views/PlansView.vue';

defineEmits<{
  logout: [];
}>();

const activeSection = ref<ConsoleSection>('images');
</script>
