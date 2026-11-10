<script setup lang="ts">
defineEmits<{ 'toggle-dark': []; 'choose-dir': []; 'setting-input': [] }>()
const darkMode = defineModel<boolean>('darkMode', { required: true })
const settingsPort = defineModel<string>('port', { required: true })
const settingsDir = defineModel<string>('dir', { required: true })
const maxConcurrent = defineModel<string>('maxConcurrent', { required: true })
const speedLimit = defineModel<string>('speedLimit', { required: true })
</script>

<template>
<div class="body">
          <div class="opt-row">
            <span class="opt-icon" :class="{ dark: darkMode }">
              <svg v-if="!darkMode" viewBox="0 0 24 24">
                <path
                  fill="currentColor"
                  d="M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10zM2 13h2a1 1 0 1 0 0-2H2a1 1 0 1 0 0 2zm18 0h2a1 1 0 1 0 0-2h-2a1 1 0 1 0 0 2zM11 2v2a1 1 0 1 0 2 0V2a1 1 0 1 0-2 0zm0 18v2a1 1 0 1 0 2 0v-2a1 1 0 1 0-2 0zM4.22 5.64l1.42 1.42a1 1 0 1 0 1.41-1.41L5.64 4.22a1 1 0 1 0-1.42 1.42zm12.72 12.73 1.42 1.42a1 1 0 1 0 1.42-1.42l-1.42-1.41a1 1 0 1 0-1.41 1.41zM5.64 19.78l1.41-1.42a1 1 0 1 0-1.41-1.41l-1.42 1.41a1 1 0 1 0 1.42 1.42zM18.36 5.64l1.42-1.42a1 1 0 1 0-1.42-1.42l-1.41 1.42a1 1 0 1 0 1.41 1.42z"
                />
              </svg>
              <svg v-else viewBox="0 0 24 24">
                <path
                  fill="currentColor"
                  d="M12.3 4.9c.4-.2.6-.7.5-1.1s-.6-.8-1-.8C6.8 3.1 3 7.1 3 12c0 5 4 9 9 9 4.9 0 8.9-3.8 9-8.8 0-.4-.3-.9-.8-1s-.9.1-1.1.5c-.9 1.3-2.4 2.3-4.1 2.3-2.8 0-5-2.2-5-5 0-1.7.9-3.2 2.3-4.1z"
                />
              </svg>
            </span>
            <span class="opt-text">
              <span class="opt-t">暗黑模式</span>
              <span class="opt-s">立即生效,重启后保持</span>
            </span>
            <button
              class="switch"
              :class="{ on: darkMode }"
              role="switch"
              :aria-checked="darkMode"
              @click="$emit('toggle-dark')"
            >
              <i class="knob"></i>
            </button>
          </div>

          <div class="hr"></div>

          <div class="field">
            <label>监听端口</label>
            <input
              v-model="settingsPort"
              class="input"
              placeholder="留空自动分配(默认 8420)"
              inputmode="numeric"
              @input="$emit('setting-input')"
            />
            <p class="sub">改动端口需重启应用生效;端口被占用时自动回落随机端口</p>
          </div>

          <div class="field">
            <label>共享目录</label>
            <div class="input-row">
              <input v-model="settingsDir" class="input" readonly placeholder="~/Pigeon/Inbox" />
              <button class="sq-btn wide" @click="$emit('choose-dir')">选择…</button>
            </div>
            <p class="sub">修改后立即生效,已分享的文件留在原目录</p>
          </div>

          <div class="hr"></div>

          <div class="field">
            <label>传输</label>
            <div class="input-row">
              <input
                v-model="maxConcurrent"
                class="input"
                type="number"
                min="1"
                max="8"
                placeholder="并发上限"
                @input="$emit('setting-input')"
              />
              <div class="input-unit">
                <input
                  v-model="speedLimit"
                  class="input"
                  type="number"
                  min="0"
                  placeholder="限速"
                  @input="$emit('setting-input')"
                />
                <span class="unit">MB/s</span>
              </div>
            </div>
            <p class="sub">并发上传上限(1–8,重启生效) · 全局限速 0 = 不限,立即生效</p>
          </div>

          <p class="sub note">
            所有设置即改即存,保存在 <code>~/Pigeon/settings.json</code> · token 每次启动随机生成
          </p>
        </div>
</template>

<style scoped>
.input-unit {
  position: relative;
  flex: 0 0 130px;
}

.input-unit .input {
  padding-right: 48px;
}

.input-unit .unit {
  position: absolute;
  right: 10px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 11px;
  color: var(--text-4);
  pointer-events: none;
}

.opt-icon {
  flex: none;
  width: 38px;
  height: 38px;
  border-radius: 10px;
  background: var(--accent-bg);
  color: var(--accent);
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.opt-icon.dark {
  background: var(--chip-bg);
  color: var(--text-2);
}

.opt-icon svg {
  width: 18px;
  height: 18px;
}

.note {
  margin-top: 16px;
}

.note code {
  font-family: ui-monospace, Consolas, monospace;
  background: var(--chip-bg);
  border-radius: 4px;
  padding: 1px 5px;
  color: var(--text-2);
}
</style>
