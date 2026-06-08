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

export interface ApiEnvelope<T> {
  code: number;
  message: string;
  data: T;
}
