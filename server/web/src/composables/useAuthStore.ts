import { computed, reactive, ref } from 'vue';
import { login as loginRequest } from '../api';
import { onUnauthorized } from '../api/client';

const tokenStorageKey = 'inkframe-admin-token';
const tokenExpiresAtStorageKey = 'inkframe-admin-token-expires-at';

function loadStoredToken() {
  const storedToken = localStorage.getItem(tokenStorageKey) || '';
  const expiresAt = localStorage.getItem(tokenExpiresAtStorageKey) || '';
  if (!storedToken || !expiresAt) {
    return '';
  }
  if (Number.isNaN(Date.parse(expiresAt)) || Date.now() >= Date.parse(expiresAt)) {
    localStorage.removeItem(tokenStorageKey);
    localStorage.removeItem(tokenExpiresAtStorageKey);
    return '';
  }
  return storedToken;
}

const token = ref(loadStoredToken());
const loggingIn = ref(false);
const loginForm = reactive({
  username: '',
  password: '',
});

const isLoggedIn = computed(() => token.value.length > 0);

async function login(username: string, password: string) {
  loggingIn.value = true;
  try {
    const result = await loginRequest(username, password);
    token.value = result.jwtToken;
    localStorage.setItem(tokenStorageKey, result.jwtToken);
    localStorage.setItem(tokenExpiresAtStorageKey, result.expiresAt);
    loginForm.password = '';
    return result;
  } finally {
    loggingIn.value = false;
  }
}

function logout() {
  token.value = '';
  localStorage.removeItem(tokenStorageKey);
  localStorage.removeItem(tokenExpiresAtStorageKey);
}

onUnauthorized(logout);

export function useAuthStore() {
  return {
    token,
    isLoggedIn,
    loginForm,
    loggingIn,
    login,
    logout,
  };
}
