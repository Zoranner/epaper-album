import { ApiError, ApiUnauthorizedError, authHeaders, readJsonEnvelope, request, tokenInit } from './client';
import type { AdminImage } from './types';

export function listImages(token: string, keyword = '', tags: string[] = []): Promise<AdminImage[]> {
  const params = new URLSearchParams();
  if (keyword.trim()) {
    params.set('keyword', keyword.trim());
  }
  if (tags.length > 0) {
    params.set('tags', tags.join(','));
  }
  const query = params.toString();

  return request<AdminImage[]>(`/images${query ? `?${query}` : ''}`, tokenInit(token));
}

export function uploadImage(token: string, file: File, remark: string, tags: string[]): Promise<AdminImage> {
  const formData = new FormData();
  formData.append('image', file);
  if (remark.trim()) {
    formData.append('remark', remark.trim());
  }
  formData.append('tags', JSON.stringify(normalizeTags(tags)));

  return request<AdminImage>(
    '/images',
    tokenInit(token, {
      method: 'POST',
      body: formData,
    }),
  );
}

export function updateImage(
  token: string,
  sha256: string,
  remark: string,
  tags: string[],
): Promise<AdminImage> {
  return request<AdminImage>(
    `/images/${encodeURIComponent(sha256)}`,
    tokenInit(token, {
      method: 'PUT',
      body: JSON.stringify({ remark, tags: normalizeTags(tags) }),
    }),
  );
}

export const updateImageRemark = updateImage;

export function deleteImage(token: string, sha256: string): Promise<null> {
  return request<null>(
    `/images/${encodeURIComponent(sha256)}`,
    tokenInit(token, {
      method: 'DELETE',
    }),
  );
}

export function reditherImage(token: string, sha256: string): Promise<AdminImage> {
  return request<AdminImage>(
    `/images/${encodeURIComponent(sha256)}/redither`,
    tokenInit(token, {
      method: 'POST',
    }),
  );
}

export async function getImageBlob(token: string, sha256: string): Promise<Blob> {
  const response = await fetch(`/images/${encodeURIComponent(sha256)}`, {
    headers: authHeaders(token),
  });

  if (!response.ok) {
    const envelope = await readJsonEnvelope<null>(response);
    if (response.status === 401) {
      throw new ApiUnauthorizedError(envelope.message || undefined);
    }
    throw new ApiError(envelope.message || `图片读取失败：${response.status}`, response.status, envelope.code);
  }

  return response.blob();
}

export function normalizeTags(tags: string[]): string[] {
  const normalized: string[] = [];
  for (const tag of tags) {
    const value = tag.trim();
    if (value && !normalized.includes(value)) {
      normalized.push(value);
    }
  }
  return normalized;
}

export function parseTagInput(value: string): string[] {
  return normalizeTags(value.split(/[,，\s]+/));
}
