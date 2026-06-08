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

export function updatePlan(token: string, date: string, payload: PlanPayload): Promise<AdminPlan> {
  return request<AdminPlan>(
    `/plans/${date}`,
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
