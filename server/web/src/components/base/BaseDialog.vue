<template>
  <Teleport to="body">
    <div v-if="open" class="base-dialog__backdrop" @click.self="$emit('close')">
      <section class="base-dialog" role="dialog" aria-modal="true" :aria-label="title">
        <header class="base-dialog__header">
          <div>
            <h2 v-if="title" class="base-dialog__title">{{ title }}</h2>
            <p v-if="description" class="base-dialog__description">{{ description }}</p>
          </div>
          <button class="base-button ghost small base-dialog__close" type="button" @click="$emit('close')">
            关闭
          </button>
        </header>
        <slot></slot>
        <footer v-if="$slots.footer" class="base-dialog__footer">
          <slot name="footer"></slot>
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
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
