<template>
  <div ref="menuRef" class="action-menu" @keydown.esc="closeMenu">
    <span ref="triggerRef" class="action-menu__trigger">
      <Button
        :aria-expanded="open"
        aria-haspopup="menu"
        small
        icon="more"
        icon-only
        label="操作"
        variant="ghost"
        @click="toggleMenu"
      />
    </span>
    <Teleport to="body">
      <div v-if="open" ref="panelRef" class="action-menu__items" :style="panelStyle" role="menu">
        <button
          v-for="item in items"
          :key="item.key"
          :aria-disabled="item.disabled"
          :class="{ danger: item.danger, disabled: item.disabled }"
          :disabled="item.disabled"
          role="menuitem"
          type="button"
          @click="selectItem(item)"
        >
          <Icon v-if="item.icon" :name="item.icon" />
          <span>{{ item.label }}</span>
        </button>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref } from 'vue';
import Button from '../base/Button.vue';
import Icon, { type IconName } from '../display/Icon.vue';

export interface ActionMenuItem {
  key: string;
  label: string;
  icon?: IconName;
  danger?: boolean;
  disabled?: boolean;
}

defineProps<{
  items: ActionMenuItem[];
}>();

const emit = defineEmits<{
  select: [key: string];
}>();

const open = ref(false);
const menuRef = ref<HTMLElement | null>(null);
const triggerRef = ref<HTMLElement | null>(null);
const panelRef = ref<HTMLElement | null>(null);
const panelStyle = ref<Record<string, string>>({});

function toggleMenu() {
  open.value = !open.value;
  if (open.value) {
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    window.addEventListener('resize', updatePanelPosition);
    window.addEventListener('scroll', updatePanelPosition, true);
    void nextTick(updatePanelPosition);
  } else {
    removeGlobalListeners();
  }
}

function closeMenu() {
  open.value = false;
  removeGlobalListeners();
}

function closeOnOutsidePointer(event: PointerEvent) {
  const target = event.target as Node;
  if (!menuRef.value?.contains(target) && !panelRef.value?.contains(target)) {
    closeMenu();
  }
}

function selectItem(item: ActionMenuItem) {
  if (item.disabled) {
    return;
  }
  closeMenu();
  emit('select', item.key);
}

function updatePanelPosition() {
  const trigger = triggerRef.value;
  if (!trigger) {
    return;
  }

  const rect = trigger.getBoundingClientRect();
  const panelWidth = panelRef.value?.offsetWidth ?? 160;
  const panelHeight = panelRef.value?.offsetHeight ?? 160;
  const left = Math.max(8, Math.min(window.innerWidth - panelWidth - 8, rect.right - panelWidth));
  let top = rect.bottom + 4;
  if (top + panelHeight > window.innerHeight - 8) {
    top = Math.max(8, rect.top - panelHeight - 4);
  }
  panelStyle.value = {
    left: `${left}px`,
    top: `${top}px`,
  };
}

function removeGlobalListeners() {
  document.removeEventListener('pointerdown', closeOnOutsidePointer);
  window.removeEventListener('resize', updatePanelPosition);
  window.removeEventListener('scroll', updatePanelPosition, true);
}

onBeforeUnmount(() => {
  removeGlobalListeners();
});
</script>
