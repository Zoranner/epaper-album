import { clampPlanDays, request, tokenInit } from './client';
import type { AdminPlan, PlanPayload } from './types';

export function listPlans(token: string, days: number): Promise<AdminPlan[]> {
  const params = new URLSearchParams({ days: String(clampPlanDays(days)) });
  return request<AdminPlan[]>(`/plans?${params.toString()}`, tokenInit(token));
}

export function createPlan(token: string, payload: PlanPayload): Promise<AdminPlan> {
  return request<AdminPlan>(
    '/plans',
    tokenInit(token, {
      method: 'POST',
      body: JSON.stringify(payload),
    }),
  );
}

export function updatePlan(token: string, id: number, payload: PlanPayload): Promise<AdminPlan> {
  return request<AdminPlan>(
    `/plans/${id}`,
    tokenInit(token, {
      method: 'PUT',
      body: JSON.stringify(payload),
    }),
  );
}

export function deletePlan(token: string, id: number): Promise<null> {
  return request<null>(
    `/plans/${id}`,
    tokenInit(token, {
      method: 'DELETE',
    }),
  );
}
