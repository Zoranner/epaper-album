import { request } from './client';
import type { LoginResponse } from './types';

export function login(username: string, password: string): Promise<LoginResponse> {
  return request<LoginResponse>('/login', {
    method: 'POST',
    body: JSON.stringify({ username, password }),
  });
}
