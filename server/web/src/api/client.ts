import type { ApiEnvelope } from './types';

const apiBase = '/api';
const unauthorizedEventName = 'epaper-album:unauthorized';

export class ApiUnauthorizedError extends Error {
  constructor(message = '登录信息已失效，请重新登录') {
    super(message);
    this.name = 'ApiUnauthorizedError';
  }
}

export function onUnauthorized(listener: () => void): () => void {
  window.addEventListener(unauthorizedEventName, listener);
  return () => window.removeEventListener(unauthorizedEventName, listener);
}

function notifyUnauthorized() {
  window.dispatchEvent(new CustomEvent(unauthorizedEventName));
}

export function authHeaders(token: string): Record<string, string> {
  return {
    Authorization: `Bearer ${token}`,
  };
}

export async function readJsonEnvelope<T>(response: Response): Promise<ApiEnvelope<T>> {
  if (response.status === 401) {
    notifyUnauthorized();
  }

  const contentType = response.headers.get('content-type') || '';
  if (!contentType.includes('application/json')) {
    const text = await response.text();
    if (response.status === 401) {
      throw new ApiUnauthorizedError();
    }
    throw new Error(text || `请求失败：${response.status}`);
  }

  return (await response.json()) as ApiEnvelope<T>;
}

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${apiBase}${path}`, {
    ...init,
    headers: {
      ...(init?.body instanceof FormData ? {} : { 'Content-Type': 'application/json' }),
      ...init?.headers,
    },
  });

  const envelope = await readJsonEnvelope<T>(response);
  if (response.status === 401) {
    throw new ApiUnauthorizedError(envelope.message || undefined);
  }
  if (!response.ok || envelope.code !== 0) {
    throw new Error(envelope.message || `请求失败：${response.status}`);
  }

  return envelope.data;
}

export function tokenInit(token: string, init?: RequestInit): RequestInit {
  return {
    ...init,
    headers: {
      ...authHeaders(token),
      ...init?.headers,
    },
  };
}

export function clampPlanDays(days: number): number {
  if (!Number.isFinite(days)) {
    return 3;
  }

  return Math.min(7, Math.max(1, Math.trunc(days)));
}
