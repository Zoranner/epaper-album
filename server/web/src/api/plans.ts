import { clampPlanDays, request, tokenInit } from './client';
import type { Plan } from './types';

export function listPlans(token: string, days: number): Promise<Plan[]> {
  const params = new URLSearchParams({ days: String(clampPlanDays(days)) });
  return request<Plan[]>(`/plans?${params.toString()}`, tokenInit(token));
}

export function createPlan(token: string, payload: Plan): Promise<Plan> {
  return request<Plan>(
    '/plans',
    tokenInit(token, {
      method: 'POST',
      body: JSON.stringify(payload),
    }),
  );
}

export function updatePlan(token: string, originalDate: string, payload: Plan): Promise<Plan> {
  return request<Plan>(
    `/plans/${originalDate}`,
    tokenInit(token, {
      method: 'PUT',
      body: JSON.stringify(payload),
    }),
  );
}

export function deletePlan(token: string, date: string): Promise<null> {
  return request<null>(
    `/plans/${date}`,
    tokenInit(token, {
      method: 'DELETE',
    }),
  );
}
