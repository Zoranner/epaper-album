<template>
  <main class="login-page">
    <form class="login-panel" @submit.prevent="submit">
      <header class="login-panel__header">
        <h1>电子相册管理台</h1>
        <p>管理员登录</p>
      </header>

      <Input
        autocomplete="username"
        label="账号"
        required
        :model-value="auth.loginForm.username"
        @update:model-value="auth.loginForm.username = $event"
      />
      <Input
        autocomplete="current-password"
        label="密码"
        required
        type="password"
        :model-value="auth.loginForm.password"
        @update:model-value="auth.loginForm.password = $event"
      />

      <p v-if="error" class="form-error">{{ error }}</p>

      <Button block :loading="auth.loggingIn.value" type="submit" variant="primary">
        登录
      </Button>
    </form>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { Button, Input } from '../components';
import { useAuthStore } from '../composables/useAuthStore';

const emit = defineEmits<{
  loggedIn: [];
}>();

const auth = useAuthStore();
const error = ref('');

async function submit() {
  error.value = '';
  try {
    await auth.login(auth.loginForm.username, auth.loginForm.password);
    emit('loggedIn');
  } catch (loginError) {
    error.value = loginError instanceof Error ? loginError.message : '登录失败';
  }
}
</script>
