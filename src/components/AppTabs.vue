<script setup lang="ts">
import type { Tab } from '../types/app'

defineProps<{ modelValue: Tab; fileCount: number }>()
defineEmits<{ 'update:modelValue': [value: Tab] }>()
</script>

<template>
<nav class="tabs">
          <button :class="{ on: modelValue === 'connect' }" @click="$emit('update:modelValue', 'connect')">连接</button>
          <button :class="{ on: modelValue === 'files' }" @click="$emit('update:modelValue', 'files')">
            文件<span v-if="fileCount" class="tab-n">{{ fileCount }}</span>
          </button>
          <button :class="{ on: modelValue === 'clips' }" @click="$emit('update:modelValue', 'clips')">记录</button>
          <button :class="{ on: modelValue === 'devices' }" @click="$emit('update:modelValue', 'devices')">设备</button>
          <button :class="{ on: modelValue === 'security' }" @click="$emit('update:modelValue', 'security')">安全</button>
          <button :class="{ on: modelValue === 'settings' }" @click="$emit('update:modelValue', 'settings')">设置</button>
        </nav>
</template>

<style scoped>
/* Tab */
.tabs {
  display: flex;
  gap: 26px;
  padding: 16px 22px 0;
  border-bottom: 1px solid var(--border-soft);
}
.tabs button {
  position: relative;
  border: 0;
  background: none;
  padding: 0 0 13px;
  font-size: 14px;
  color: var(--text-3);
  cursor: pointer;
  font-family: inherit;
  transition: color 0.15s;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.tabs button:hover {
  color: var(--text-2);
}
.tabs button.on {
  color: var(--text);
  font-weight: 600;
}
.tabs button.on::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 2px;
  border-radius: 2px 2px 0 0;
  background: var(--accent);
}
.tab-n {
  font-size: 11px;
  font-weight: 500;
  color: var(--text-3);
  background: var(--chip-n);
  border-radius: 999px;
  padding: 1px 7px;
  line-height: 1.6;
}
</style>
