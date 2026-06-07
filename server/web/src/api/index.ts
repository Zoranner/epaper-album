export interface AlbumPlan {
  start: string;
  end: string;
  caption: string;
  images: string[];
}

export interface Manifest {
  version: string;
  plans: AlbumPlan[];
}

export interface ImageResource {
  sha256: string;
  url: string;
  size?: number;
  content_type?: string;
  created_at?: string;
}

export interface UploadImageResponse {
  sha256: string;
  url?: string;
}

const apiBase = '/api';

async function request<T>(path: string, init?: RequestInit, secretKey?: string): Promise<T> {
  const response = await fetch(`${apiBase}${path}`, {
    ...init,
    headers: {
      ...(init?.body instanceof FormData ? {} : { 'Content-Type': 'application/json' }),
      ...(secretKey ? { 'secret-key': secretKey } : {}),
      ...init?.headers,
    },
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `请求失败：${response.status}`);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

function normalizeImageResource(item: ImageResource | string): ImageResource {
  if (typeof item === 'string') {
    return {
      sha256: item,
      url: `/images/${item}`,
    };
  }

  return {
    ...item,
    url: item.url || `/images/${item.sha256}`,
  };
}

export const albumApi = {
  getManifest: (secretKey: string) => request<Manifest>('/manifest', undefined, secretKey),
  updateManifest: (manifest: Manifest, secretKey: string) =>
    request<Manifest>('/manifest', {
      method: 'PUT',
      body: JSON.stringify(manifest),
    }, secretKey),
  uploadImage: (file: File, secretKey: string) => {
    const formData = new FormData();
    formData.append('image', file);

    return request<UploadImageResponse>('/images', {
      method: 'POST',
      body: formData,
    }, secretKey);
  },
  async listImages(secretKey: string) {
    const result = await request<Array<ImageResource | string>>('/images', undefined, secretKey);
    return result.map(normalizeImageResource);
  },
  deleteImage: (sha256: string, secretKey: string) =>
    request<void>(`/images/${encodeURIComponent(sha256)}`, {
      method: 'DELETE',
    }, secretKey),
  async getImageObjectUrl(sha256: string, secretKey: string) {
    const response = await fetch(`/images/${encodeURIComponent(sha256)}`, {
      headers: { 'secret-key': secretKey },
    });
    if (!response.ok) {
      const text = await response.text();
      throw new Error(text || `图片读取失败：${response.status}`);
    }
    return URL.createObjectURL(await response.blob());
  },
};
