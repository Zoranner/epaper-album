export type ImageStatus = 'pending' | 'processing' | 'ready' | 'failed';

export interface AdminImage {
  sha256: string;
  status: ImageStatus;
  remark: string;
}

export interface AdminPlan {
  id: number;
  start: string;
  end: string;
  caption: string;
  images: AdminImage[];
}

export interface PlanPayload {
  start: string;
  end: string;
  caption: string;
  images: string[];
}

export interface LoginResponse {
  jwtToken: string;
  expiresAt: string;
}

interface ApiEnvelope<T> {
  code: number;
  message: string;
  data: T;
}

const apiBase = '/api';

function authHeaders(token: string): Record<string, string> {
  return {
    Authorization: `Bearer ${token}`,
  };
}

async function readJsonEnvelope<T>(response: Response): Promise<ApiEnvelope<T>> {
  const contentType = response.headers.get('content-type') || '';
  if (!contentType.includes('application/json')) {
    const text = await response.text();
    throw new Error(text || `请求失败：${response.status}`);
  }

  return (await response.json()) as ApiEnvelope<T>;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
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

function tokenInit(token: string, init?: RequestInit): RequestInit {
  return {
    ...init,
    headers: {
      ...authHeaders(token),
      ...init?.headers,
    },
  };
}

export const albumApi = {
  login(username: string, password: string) {
    return request<LoginResponse>('/login', {
      method: 'POST',
      body: JSON.stringify({ username, password }),
    });
  },

  listImages(token: string, keyword = '') {
    const params = new URLSearchParams();
    if (keyword.trim()) {
      params.set('keyword', keyword.trim());
    }
    const query = params.toString();
    return request<AdminImage[]>(`/images${query ? `?${query}` : ''}`, tokenInit(token));
  },

  uploadImage(token: string, file: File, remark: string) {
    const formData = new FormData();
    formData.append('image', file);
    if (remark.trim()) {
      formData.append('remark', remark.trim());
    }

    return request<AdminImage>(
      '/images',
      tokenInit(token, {
        method: 'POST',
        body: formData,
      }),
    );
  },

  updateImageRemark(token: string, sha256: string, remark: string) {
    return request<AdminImage>(
      `/images/${encodeURIComponent(sha256)}`,
      tokenInit(token, {
        method: 'PUT',
        body: JSON.stringify({ remark }),
      }),
    );
  },

  listPlans(token: string, days: number) {
    const params = new URLSearchParams({ days: String(days) });
    return request<AdminPlan[]>(`/plans?${params.toString()}`, tokenInit(token));
  },

  createPlan(token: string, payload: PlanPayload) {
    return request<AdminPlan>(
      '/plans',
      tokenInit(token, {
        method: 'POST',
        body: JSON.stringify(payload),
      }),
    );
  },

  updatePlan(token: string, id: number, payload: PlanPayload) {
    return request<AdminPlan>(
      `/plans/${id}`,
      tokenInit(token, {
        method: 'PUT',
        body: JSON.stringify(payload),
      }),
    );
  },

  deletePlan(token: string, id: number) {
    return request<null>(
      `/plans/${id}`,
      tokenInit(token, {
        method: 'DELETE',
      }),
    );
  },

  async getImageObjectUrl(token: string, sha256: string) {
    const response = await fetch(`/images/${encodeURIComponent(sha256)}`, {
      headers: authHeaders(token),
    });

    if (!response.ok) {
      const envelope = await readJsonEnvelope<null>(response);
      throw new Error(envelope.message || `图片读取失败：${response.status}`);
    }

    return URL.createObjectURL(await response.blob());
  },
};
