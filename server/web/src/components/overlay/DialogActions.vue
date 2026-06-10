<template>
  <div class="dialog-actions">
    <div v-if="$slots.meta" class="dialog-actions__meta">
      <slot name="meta"></slot>
    </div>
    <div class="dialog-actions__buttons">
      <slot></slot>
    </div>
  </div>
</template>
