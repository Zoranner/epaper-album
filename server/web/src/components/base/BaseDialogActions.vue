<template>
  <div class="base-dialog-actions">
    <div v-if="$slots.meta" class="base-dialog-actions__meta">
      <slot name="meta"></slot>
    </div>
    <div class="base-dialog-actions__buttons">
      <slot></slot>
    </div>
  </div>
</template>
