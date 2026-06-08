import { computed, reactive, ref } from 'vue';
import { login as loginRequest } from '../api';

const tokenStorageKey = 'epaper-album-admin-token';
const tokenExpiresAtStorageKey = 'epaper-album-admin-token-expires-at';

const token = ref(localStorage.getItem(tokenStorageKey) || '');
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
