<script setup lang="ts">
import { ref } from 'vue'
import type { ServerInfo } from '../types/app'

defineProps<{ sharing: boolean; qr: string; serverInfo: ServerInfo | null; sending: boolean }>()
defineEmits<{ 'copy-url': []; 'send-text': []; 'send-clipboard-text': []; 'toggle-sharing': []; 'open-firewall': [] }>()
const textInput = defineModel<string>('textInput', { required: true })
const showHelp = ref(false)
</script>

<template>
<div class="body">
          <div class="connect-wrap" :class="{ off: !sharing }">
            <div class="qr-row">
              <img v-if="qr" class="qr" :src="qr" alt="扫码连接" />
              <div v-else class="qr qr-loading"></div>
              <div class="qr-side">
                <p class="cap">手机扫码连接</p>
                <div class="input-row">
                  <div class="input readonly">
                    <svg class="lead" viewBox="0 0 24 24">
                      <path
                        fill="currentColor"
                        d="M12 1a5 5 0 0 0-5 5v3H6a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-9a2 2 0 0 0-2-2h-1V6a5 5 0 0 0-5-5zm-3 8V6a3 3 0 1 1 6 0v3H9z"
                      />
                    </svg>
                    <span class="url">{{ serverInfo?.url ?? '…' }}</span>
                  </div>
                  <button
                    class="sq-btn"
                    title="复制链接"
                    :disabled="!sharing"
                    @click="$emit('copy-url')"
                  >
                    <svg viewBox="0 0 24 24">
                      <path
                        fill="currentColor"
                        d="M16 1H4a2 2 0 0 0-2 2v14h2V3h12V1zm3 4H8a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2zm0 16H8V7h11v14z"
                      />
                    </svg>
                  </button>
                </div>
                <p class="sub">手机无需安装,浏览器打开即用</p>
              </div>
            </div>

            <div class="hr"></div>

            <div class="field">
              <label>发送文字到手机</label>
              <div class="input-row">
                <input
                  v-model="textInput"
                  class="input"
                  :disabled="!sharing"
                  placeholder="输入文字,回车发送"
                  @keydown.enter.exact.prevent="$emit('send-text')"
                />
                <button
                  class="sq-btn wide"
                  :disabled="!textInput.trim() || sending || !sharing"
                  @click="$emit('send-text')"
                >
                  发送
                </button>
              </div>
              <p class="sub">
                也可以
                <button class="link" :disabled="!sharing" @click="$emit('send-clipboard-text')">
                  直接发送剪贴板内容
                </button>
              </p>
            </div>

            <!-- 关闭时的遮罩:灰化内容 + 居中开启入口 -->
            <div v-if="!sharing" class="off-overlay">
              <p class="off-t">分享已关闭</p>
              <p class="off-s">手机端无法访问,二维码与链接已失效</p>
              <button class="btn primary" @click="$emit('toggle-sharing')">开启分享</button>
            </div>
          </div>

          <!-- 连接排查助手 -->
          <p class="sub" style="text-align: center; margin-top: 10px">
            <button class="link" @click="showHelp = !showHelp">
              {{ showHelp ? '收起排查步骤' : '手机连不上?查看排查步骤' }}
            </button>
          </p>
          <div v-if="showHelp" class="help-card">
            <ol class="help-list">
              <li>确认手机和电脑连的是<b>同一个 Wi-Fi</b>(部分路由器开了 AP 隔离会互相不可见)</li>
              <li>检查顶栏状态灯为绿色「分享中」</li>
              <li>Windows 防火墙首次弹窗时需点过「允许」;没点过可在防火墙设置里放行 Pigeon</li>
            </ol>
            <button class="btn ghost" @click="$emit('open-firewall')">打开防火墙设置</button>
          </div>
        </div>
</template>

<style scoped>
/* ========== 连接 ========== */
.connect-wrap {
  position: relative;
  transition:
    opacity 0.2s,
    filter 0.2s;
}
.connect-wrap.off {
  opacity: 0.45;
  filter: grayscale(1);
  pointer-events: none;
}
.connect-wrap.off .off-overlay {
  pointer-events: auto;
}
.off-overlay {
  position: absolute;
  inset: -6px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border-radius: 12px;
  background: var(--card);
}
.off-t {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-2);
}
.off-s {
  font-size: 12px;
  color: var(--text-3);
  margin-bottom: 8px;
}
.qr-row {
  display: flex;
  gap: 20px;
  align-items: center;
}
.qr {
  width: 148px;
  height: 148px;
  border-radius: 10px;
  border: 1px solid var(--border-soft);
  background: #fff;
  flex: none;
}
.qr-loading {
  background: linear-gradient(100deg, var(--field-bg) 40%, var(--hover) 50%, var(--field-bg) 60%);
  background-size: 200% 100%;
  animation: shimmer 1.2s infinite;
}
@keyframes shimmer {
  to {
    background-position: -200% 0;
  }
}
.qr-side {
  flex: 1;
  min-width: 0;
}

.help-card {
  margin-top: 10px;
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  align-items: flex-start;
}

.help-list {
  margin: 0;
  padding-left: 18px;
  font-size: 12.5px;
  color: var(--text-2);
  line-height: 1.9;
  user-select: text;
}

.help-list b {
  color: var(--text);
}
</style>
