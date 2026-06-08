<template>
  <component :is="iconComponent" class="base-icon" aria-hidden="true" />
</template>

<script setup lang="ts">
import { computed } from 'vue';
import {
  ChevronDown,
  Edit3,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  Upload,
  X,
} from 'lucide-vue-next';

const props = defineProps<{
  name:
    | 'chevron-down'
    | 'more'
    | 'plus'
    | 'search'
    | 'upload'
    | 'edit'
    | 'trash'
    | 'refresh'
    | 'close';
}>();

const iconComponent = computed(() => {
  const icons = {
    'chevron-down': ChevronDown,
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
