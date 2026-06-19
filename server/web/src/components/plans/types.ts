import type { AdminImage, Plan } from '../../api';

export interface PlanView extends Plan {
  imageRecord: AdminImage | null;
}

export interface CalendarDay {
  date: string;
  day: string;
  inMonth: boolean;
  plan: PlanView | null;
}
