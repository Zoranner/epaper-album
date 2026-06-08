import type { ApiEnvelope } from './types';

const apiBase = '/api';

export function authHeaders(token: string): Record<string, string> {
  return {
    Authorization: `Bearer ${token}`,
  };
}

export async function readJsonEnvelope<T>(response: Response): Promise<ApiEnvelope<T>> {
  const contentType = response.headers.get('content-type') || '';
  if (!contentType.includes('application/json')) {
    const text = await response.text();
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
