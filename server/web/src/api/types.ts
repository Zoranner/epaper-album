export type ImageStatus = 'pending' | 'processing' | 'ready' | 'failed';

export interface AdminImage {
  sha256: string;
  status: ImageStatus;
  remark: string;
}

export interface Plan {
  date: string;
  caption: string;
  image: string;
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
