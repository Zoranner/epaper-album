<template>
  <LoginPage v-if="!isLoggedIn" @logged-in="handleLoggedIn" />
  <ConsolePage v-else @logout="handleLogout" />
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useAuthStore } from './composables/useAuthStore';
import ConsolePage from './pages/ConsolePage.vue';
import LoginPage from './pages/LoginPage.vue';

const auth = useAuthStore();
const isLoggedIn = computed(() => auth.token.value.length > 0);

function handleLoggedIn() {
  // Login state is stored in useAuthStore.
}

function handleLogout() {
  auth.logout();
}
</script>
