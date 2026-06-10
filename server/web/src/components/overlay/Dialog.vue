<template>
  <Teleport to="body">
    <div v-if="open" class="dialog__backdrop" @click.self="$emit('close')">
      <section class="dialog" role="dialog" aria-modal="true" :aria-label="title">
        <header class="dialog__header">
          <div>
            <h2 v-if="title" class="dialog__title">{{ title }}</h2>
            <p v-if="description" class="dialog__description">{{ description }}</p>
          </div>
          <Button icon="close" icon-only label="关闭" variant="ghost" @click="$emit('close')" />
        </header>
        <slot></slot>
        <footer v-if="$slots.footer" class="dialog__footer">
          <slot name="footer"></slot>
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import Button from '../base/Button.vue';

withDefaults(
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
</script>
