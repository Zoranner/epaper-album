<template>
  <component :is="iconComponent" class="icon" aria-hidden="true" />
</template>

<script setup lang="ts">
import { computed } from 'vue';
import {
  CalendarDays,
  Images,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Edit3,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  Upload,
  X,
} from 'lucide-vue-next';

export type IconName =
  | 'calendar'
  | 'chevron-down'
  | 'chevron-left'
  | 'chevron-right'
  | 'images'
  | 'more'
  | 'plus'
  | 'search'
  | 'upload'
  | 'edit'
  | 'trash'
  | 'refresh'
  | 'close';

const props = defineProps<{
  name: IconName;
}>();

const iconComponent = computed(() => {
  const icons = {
    calendar: CalendarDays,
    'chevron-down': ChevronDown,
    'chevron-left': ChevronLeft,
    'chevron-right': ChevronRight,
    images: Images,
    more: MoreHorizontal,
    plus: Plus,
    search: Search,
    upload: Upload,
    edit: Edit3,
    trash: Trash2,
    refresh: RefreshCw,
    close: X,
  };
  return icons[props.name];
});
</script>
