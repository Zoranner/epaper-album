<template>
  <Teleport to="body">
    <div v-if="open" class="dialog__backdrop" @click.self="$emit('close')">
      <section class="dialog" role="dialog" aria-modal="true" :aria-label="title">
        <header class="dialog__header">
          <div>
            <h2 v-if="title" class="dialog__title">{{ title }}</h2>
            <p v-if="description" class="dialog__description">{{ description }}</p>
          </div>
          <div class="dialog__header-actions">
            <slot name="actions"></slot>
            <Button icon="close" icon-only label="关闭" variant="ghost" @click="$emit('close')" />
          </div>
        </header>
        <div class="dialog__body">
          <slot></slot>
        </div>
        <footer v-if="$slots.footer" class="dialog__footer">
          <slot name="footer"></slot>
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { onBeforeUnmount, watch } from 'vue';
import Button from '../base/Button.vue';
import { lockDialogScroll, unlockDialogScroll } from './dialogScrollLock';

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string;
    description?: string;
  }>(),
  {
    title: '',
    description: '',
  },
);

defineEmits<{
  close: [];
}>();

watch(
  () => props.open,
  (open, wasOpen) => {
    if (open && !wasOpen) {
      lockDialogScroll();
      return;
    }
    if (!open && wasOpen) {
      unlockDialogScroll();
    }
  },
);

onBeforeUnmount(() => {
  if (props.open) {
    unlockDialogScroll();
  }
});
</script>
