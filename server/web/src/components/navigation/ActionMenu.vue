<template>
  <div ref="menuRef" class="action-menu" @keydown.esc="closeMenu">
    <Button
      :aria-expanded="open"
      aria-haspopup="menu"
      icon="more"
      icon-only
      label="操作"
      variant="ghost"
      @click="toggleMenu"
    />
    <div v-if="open" class="action-menu__items" role="menu">
      <button
        v-for="item in items"
        :key="item.key"
        :class="{ danger: item.danger }"
        role="menuitem"
        type="button"
        @click="selectItem(item.key)"
      >
        <Icon v-if="item.icon" :name="item.icon" />
        <span>{{ item.label }}</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue';
import Button from '../base/Button.vue';
import Icon, { type IconName } from '../display/Icon.vue';

export interface ActionMenuItem {
  key: string;
  label: string;
  icon?: IconName;
  danger?: boolean;
}

defineProps<{
  items: ActionMenuItem[];
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
