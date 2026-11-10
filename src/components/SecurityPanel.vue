<script setup lang="ts">
import type { AccountView } from '../types/app'

defineProps<{ tokenEnabled: boolean; authEnabled: boolean; accounts: AccountView[] }>()
defineEmits<{ 'toggle-token': []; 'toggle-auth': []; 'remove-account': [username: string]; 'open-account-dialog': [] }>()
</script>

<template>
<div class="body">
          <div class="field">
            <label>访问控制</label>
            <div class="opt-row compact">
              <span class="opt-text">
                <span class="opt-t">链接 token</span>
                <span class="opt-s">链接中携带随机 token,不知道完整链接无法访问</span>
              </span>
              <button
                class="switch"
                :class="{ on: tokenEnabled }"
                role="switch"
                :aria-checked="tokenEnabled"
                @click="$emit('toggle-token')"
              >
                <i class="knob"></i>
              </button>
            </div>
            <div class="opt-row compact">
              <span class="opt-text">
                <span class="opt-t">账号认证</span>
                <span class="opt-s">手机端需输入账号密码才能上传/下载</span>
              </span>
              <button
                class="switch"
                :class="{ on: authEnabled }"
                role="switch"
                :aria-checked="authEnabled"
                @click="$emit('toggle-auth')"
              >
                <i class="knob"></i>
              </button>
            </div>
            <p v-if="!tokenEnabled" class="sub warn">
              已关闭 token:局域网内任何人拿到 IP 即可访问,请确认网络可信
            </p>
          </div>

          <div class="hr"></div>

          <div class="field">
            <label>账号</label>
            <p v-if="!accounts.length" class="sub" style="margin: 0 0 10px">
              还没有账号。添加后打开上方的「账号认证」开关,手机端就需要登录才能访问。
            </p>
            <ul class="acc-list">
              <li v-for="a in accounts" :key="a.username">
                <span class="acc-name">{{ a.username }}</span>
                <span class="perm-chips">
                  <span class="chip" :class="{ off: !a.allow_download }">下载</span>
                  <span class="chip" :class="{ off: !a.allow_upload_file }">上传文件</span>
                  <span class="chip" :class="{ off: !a.allow_upload_text }">上传文本</span>
                  <span class="chip" :class="{ off: !a.allow_delete }">删除</span>
                </span>
                <button class="op danger" title="删除账号" @click="$emit('remove-account', a.username)">
                  <svg viewBox="0 0 24 24">
                    <path
                      fill="currentColor"
                      d="M6 19a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                    />
                  </svg>
                </button>
              </li>
            </ul>
            <button class="btn primary" @click="$emit('open-account-dialog')">添加账号</button>
            <p class="sub">
              账号保存在 <code>~/Pigeon/accounts.json</code> · 密码以加盐哈希存储
            </p>
          </div>
        </div>
</template>

<style scoped>
.opt-row.compact {
  padding: 9px 0;
  gap: 10px;
  border-bottom: 1px solid var(--border-row);
}

.opt-row.compact:last-of-type {
  border-bottom: 0;
  padding-bottom: 0;
}

.opt-row.compact .opt-t {
  font-weight: 500;
  font-size: 12.5px;
}

.opt-row.compact .opt-s {
  font-size: 11.5px;
}

.warn {
  color: var(--warning-text);
  background: var(--warning-bg);
  border-radius: 8px;
  padding: 8px 10px;
  margin-top: 10px;
}


.acc-list {
  list-style: none;
  margin: 0 0 10px;
  padding: 0;
}

.acc-list li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  border-bottom: 1px solid var(--border-row);
}

.acc-list li:last-child {
  border-bottom: 0;
}

.acc-name {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text);
}

.perm-chips {
  flex: 1;
  display: flex;
  gap: 5px;
  justify-content: flex-end;
  flex-wrap: wrap;
}

.chip {
  font-size: 10.5px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--accent-bg);
  color: var(--accent);
  white-space: nowrap;
}

.chip.off {
  background: var(--chip-bg);
  color: var(--text-4);
  text-decoration: line-through;
}
</style>
