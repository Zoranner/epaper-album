import type { AdminImage, Plan } from '../../api';

export interface PlanView extends Plan {
  imageRecord: AdminImage | null;
}
