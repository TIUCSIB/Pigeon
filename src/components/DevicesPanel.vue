<script setup lang="ts">
import type { DeviceEntry, ServerInfo, TransferRecord } from '../types/app'
import { fmtSize, timeAgo } from '../utils/format'

const props = defineProps<{
  devices: DeviceEntry[]
  history: TransferRecord[]
  serverInfo: ServerInfo | null
}>()
defineEmits<{ 'clear-history': [] }>()

function isSelf(d: DeviceEntry): boolean {
  return d.ip === props.serverInfo?.host
}

function isSelfIp(ip: string): boolean {
  return ip === props.serverInfo?.host
}

// 从 UA 猜一个简短设备名(手机端 UA 里带系统名)
function uaLabel(ua: string): string {
  if (!ua) return '未知设备'
  if (/iphone/i.test(ua)) return 'iPhone'
  if (/ipad/i.test(ua)) return 'iPad'
  if (/android/i.test(ua)) return /mobile/i.test(ua) ? 'Android 手机' : 'Android 平板'
  if (/windows/i.test(ua)) return 'Windows'
  if (/macintosh|mac os/i.test(ua)) return 'Mac'
  if (/linux/i.test(ua)) return 'Linux'
  return '浏览器'
}
</script>

<template>
<div class="body">
          <!-- 连接过的设备 -->
          <div class="list-head">
            <span class="sec">连接过的设备</span>
            <span class="sec-n">{{ devices.length }}</span>
          </div>
          <ul v-if="devices.length" class="list">
            <li v-for="d in devices" :key="d.ip">
              <span class="dot" :class="{ on: d.sse }" :title="d.sse ? '在线' : '离线'"></span>
              <span class="dev-main">
                <span class="dev-name">
                  {{ isSelf(d) ? '本机' : uaLabel(d.ua) }}
                  <span class="dev-ip">{{ d.ip }}</span>
                </span>
                <span class="dev-sub">
                  {{ d.sse ? '在线' : `${timeAgo(d.last_seen)}活跃` }} · 收 {{ d.uploads }} · 发
                  {{ d.downloads }}
                  <template v-if="d.username"> · {{ d.username }}</template>
                </span>
              </span>
            </li>
          </ul>
          <p v-else class="empty">手机连上后会出现在这里</p>

          <!-- 传输历史 -->
          <div class="list-head hist-head">
            <span class="sec">传输历史</span>
            <span class="sec-n">{{ history.length }}</span>
            <button v-if="history.length" class="link danger-link" @click="$emit('clear-history')">
              清空
            </button>
          </div>
          <ul v-if="history.length" class="list">
            <li v-for="r in history" :key="r.id">
              <span class="dir" :class="r.dir">{{ r.dir === 'in' ? '↓' : '↑' }}</span>
              <span class="dev-main">
                <span class="dev-name">{{ r.name }}</span>
                <span class="dev-sub">
                  {{ r.dir === 'in' ? '收到' : '发出' }} · {{ fmtSize(r.size) }} ·
                  {{ timeAgo(r.time) }} · {{ isSelfIp(r.ip) ? '本机' : r.ip }}
                </span>
              </span>
            </li>
          </ul>
          <p v-else class="empty">还没有传输记录</p>
        </div>
</template>

<style scoped>
.hist-head {
  margin-top: 22px;
}
.dev-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 6px 0;
}
.dev-name {
  font-size: 13px;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.dev-ip {
  font-size: 11px;
  color: var(--text-4);
  margin-left: 6px;
  font-family: ui-monospace, Consolas, monospace;
}
.dev-sub {
  font-size: 11.5px;
  color: var(--text-3);
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-4);
  flex: none;
}
.dot.on {
  background: var(--ok);
}
.dir {
  flex: none;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
}
.dir.in {
  color: var(--accent);
  background: var(--accent-bg);
}
.dir.out {
  color: var(--ok);
  background: var(--field-bg);
}
</style>
