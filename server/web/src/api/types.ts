export type ImageStatus = 'pending' | 'processing' | 'ready' | 'failed';

export interface AdminImage {
  sha256: string;
  status: ImageStatus;
  remark: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

export type PlanType = 'fixed' | 'random';

export interface Plan {
  date: string;
  caption: string;
  type?: PlanType;
  image: string;
  tags?: string[];
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
