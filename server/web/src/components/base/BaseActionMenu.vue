<template>
  <div ref="menuRef" class="base-action-menu" @keydown.esc="closeMenu">
    <BaseIconButton
      :aria-expanded="open"
      aria-haspopup="menu"
      icon="more"
      label="操作"
      @click="toggleMenu"
    />
    <div v-if="open" class="base-action-menu__items" role="menu">
      <button
        v-for="item in items"
        :key="item.key"
        :class="{ danger: item.danger }"
        role="menuitem"
        type="button"
        @click="selectItem(item.key)"
      >
        <BaseIcon v-if="item.icon" :name="item.icon" />
        <span>{{ item.label }}</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue';
import BaseIcon from './BaseIcon.vue';
import BaseIconButton from './BaseIconButton.vue';

export interface BaseActionMenuItem {
  key: string;
  label: string;
  icon?: InstanceType<typeof BaseIcon>['$props']['name'];
  danger?: boolean;
}

defineProps<{
  items: BaseActionMenuItem[];
}>();

const emit = defineEmits<{
  select: [key: string];
}>();

const open = ref(false);
const menuRef = ref<HTMLElement | null>(null);

function toggleMenu() {
  open.value = !open.value;
  if (open.value) {
    document.addEventListener('pointerdown', closeOnOutsidePointer);
  } else {
    document.removeEventListener('pointerdown', closeOnOutsidePointer);
  }
}

function closeMenu() {
  open.value = false;
  document.removeEventListener('pointerdown', closeOnOutsidePointer);
}

function closeOnOutsidePointer(event: PointerEvent) {
  if (!menuRef.value?.contains(event.target as Node)) {
    closeMenu();
  }
}

function selectItem(key: string) {
  closeMenu();
  emit('select', key);
}

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', closeOnOutsidePointer);
});
</script>
