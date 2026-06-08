import type { AdminImage, AdminPlan } from '../../api';

export interface PlanView extends AdminPlan {
  image: AdminImage | null;
}
