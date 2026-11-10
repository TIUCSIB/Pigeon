<script setup lang="ts">
import type { Toast } from '../types/app'

defineProps<{ toasts: Toast[] }>()
</script>

<template>
  <div class="toasts">
      <TransitionGroup name="toast">
        <div v-for="t in toasts" :key="t.id" class="toast">{{ t.text }}</div>
      </TransitionGroup>
    </div>
</template>

<style scoped>
/* ========== Toast ========== */
.toasts {
  position: fixed;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: center;
  z-index: 50;
}
.toast {
  background: var(--toast-bg);
  color: #fff;
  font-size: 12.5px;
  padding: 10px 17px;
  border-radius: 999px;
  max-width: 90vw;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toast-enter-active,
.toast-leave-active {
  transition: all 0.25s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
