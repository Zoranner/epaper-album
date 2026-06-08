export type ImageStatus = 'pending' | 'processing' | 'ready' | 'failed';

export interface AdminImage {
  sha256: string;
  status: ImageStatus;
  remark: string;
}

export interface AdminPlan {
  date: string;
  caption: string;
  image_sha256: string;
}

export interface PlanPayload {
  date: string;
  caption: string;
  image_sha256: string;
}

export interface LoginResponse {
  jwtToken: string;
  expiresAt: string;
}

export interface ApiEnvelope<T> {
  code: number;
  message: string;
  data: T;
}
