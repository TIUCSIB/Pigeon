<script setup lang="ts">
defineProps<{ sharing: boolean }>()
defineEmits<{ 'toggle-sharing': []; minimize: []; close: [] }>()
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <span class="brand-logo" data-tauri-drag-region>
        <img src="/logo.png" alt="Pigeon" data-tauri-drag-region draggable="false" />
      </span>
      <span class="brand-name" data-tauri-drag-region>Pigeon</span>
    </div>
    <div class="titlebar-right">
      <button
        class="status"
        :class="{ off: !sharing }"
        :title="sharing ? '分享开启中,点击关闭' : '分享已关闭,点击开启'"
        @click="$emit('toggle-sharing')"
      >
        <i></i>{{ sharing ? '分享中' : '已关闭' }}
      </button>
      <div class="win-btns">
        <button class="win-btn" title="最小化" @click="$emit('minimize')">
          <svg viewBox="0 0 10 10"><path d="M0 5h10" stroke="currentColor" stroke-width="1" /></svg>
        </button>
        <button class="win-btn close" title="关闭" @click="$emit('close')">
          <svg viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" /></svg>
        </button>
      </div>
    </div>
  </header>
</template>

<style scoped>
/* 自绘标题栏:通到窗口边缘,整行可拖拽 */
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 46px;
  margin: 0 -8px; /* 抵消页面左右 padding(与 .page 的 8px 对应) */
  padding-left: 8px;
  flex: none;
}
.titlebar-right {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.win-btns {
  display: flex;
  align-self: stretch;
  height: 46px;
}
.win-btn {
  width: 46px;
  border: 0;
  background: none;
  color: var(--text-2);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.win-btn svg {
  width: 10px;
  height: 10px;
}
.win-btn:hover {
  background: var(--hover);
  color: var(--text);
}
.win-btn.close:hover {
  background: #e5484d;
  color: #fff;
}

/* 顶部品牌行 */
.brand {
  display: flex;
  align-items: center;
  gap: 9px;
}
.brand-logo {
  width: 26px;
  height: 26px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.brand-logo img {
  width: 26px;
  height: 26px;
  -webkit-user-drag: none;
  user-select: none;
}
.brand-name {
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0.2px;
  color: var(--text);
}
.status {
  font-size: 12px;
  color: var(--text-2);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-family: inherit;
  border: 0;
  background: none;
  padding: 4px 8px;
  border-radius: 999px;
  cursor: pointer;
  transition: background 0.15s;
}
.status:hover {
  background: var(--hover);
}
.status i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--ok);
  /* 开启状态:绿色呼吸 */
  animation: breath 2.2s ease-in-out infinite;
}
.status.off i {
  background: var(--danger);
  animation: none;
}
@keyframes breath {
  0%,
  100% {
    box-shadow: 0 0 0 0 rgba(34, 160, 107, 0.45);
    opacity: 1;
  }
  50% {
    box-shadow: 0 0 0 4px rgba(34, 160, 107, 0);
    opacity: 0.75;
  }
}
</style>
