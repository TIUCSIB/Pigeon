<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import QRCode from 'qrcode'

interface ServerInfo {
  url: string
  host: string
  port: number
  token: string
  inbox_dir: string
}

interface FileEntry {
  name: string
  size: number
  modified: number
}

interface Clip {
  id: number
  content: string
  created: number
  source: string
}

interface Settings {
  port: number
  inbox_dir: string | null
}

type Tab = 'connect' | 'files' | 'clips' | 'settings'

const tab = ref<Tab>('connect')
const info = ref<ServerInfo | null>(null)
const qr = ref('')
const files = ref<FileEntry[]>([])
const clips = ref<Clip[]>([])
const dragging = ref(false)
const uploading = ref(false)
const textInput = ref('')
const sending = ref(false)
const toasts = ref<{ id: number; text: string }[]>([])
const deleteTarget = ref<string | null>(null)
// 桌面 UI 自己调 HTTP 上传接口(与手机端同通道)
const uploadUrl = ref('')

// 设置
const settingsPort = ref('')
const settingsDir = ref('')
const savingSettings = ref(false)
const settingsLoaded = ref(false)

let toastSeq = 0
const unlisteners: UnlistenFn[] = []

function fmtSize(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`
  return `${(n / 1024 ** 3).toFixed(2)} GB`
}

function timeAgo(ms: number): string {
  const s = Math.max(0, (Date.now() - ms) / 1000)
  if (s < 60) return '刚刚'
  if (s < 3600) return `${Math.floor(s / 60)} 分钟前`
  if (s < 86400) return `${Math.floor(s / 3600)} 小时前`
  return `${Math.floor(s / 86400)} 天前`
}

function fmtClipTime(ms: number): string {
  const d = new Date(ms)
  const hm = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
  const now = new Date()
  if (d.toDateString() === now.toDateString()) return hm
  return `${d.getMonth() + 1}/${d.getDate()} ${hm}`
}

function extOf(name: string): string {
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(i + 1).toLowerCase().slice(0, 5) : 'file'
}

function toast(text: string) {
  const id = ++toastSeq
  toasts.value.push({ id, text })
  setTimeout(() => {
    toasts.value = toasts.value.filter((t) => t.id !== id)
  }, 3600)
}

async function refreshFiles() {
  try {
    files.value = await invoke<FileEntry[]>('list_files')
  } catch {
    // 服务尚未就绪时忽略
  }
}

async function refreshClips() {
  try {
    clips.value = await invoke<Clip[]>('list_clips')
  } catch {
    // ignore
  }
}

async function sharePaths(paths: string[]) {
  if (paths.length === 0) return
  try {
    await invoke('share_files', { paths })
  } catch (e) {
    toast(`${e}`)
  }
}

async function browse() {
  const picked = await open({ multiple: true })
  if (!picked) return
  await sharePaths(Array.isArray(picked) ? picked : [picked])
}

async function copyRaw(text: string) {
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    const ta = document.createElement('textarea')
    ta.value = text
    document.body.appendChild(ta)
    ta.select()
    document.execCommand('copy')
    ta.remove()
  }
}

async function copyUrl() {
  if (!info.value) return
  await copyRaw(info.value.url)
  toast('链接已复制')
}

async function copyText(text: string) {
  await copyRaw(text)
  toast('已复制')
}

async function copyFileUrl(name: string) {
  try {
    const url = await invoke<string>('file_url', { name })
    await copyRaw(url)
    toast('下载链接已复制')
  } catch (e) {
    toast(`${e}`)
  }
}

async function openFolder() {
  try {
    await invoke('open_folder')
  } catch (e) {
    toast(`${e}`)
  }
}

async function reveal(name: string) {
  try {
    await invoke('reveal_file', { name })
  } catch (e) {
    toast(`${e}`)
  }
}

async function doDelete() {
  const name = deleteTarget.value
  if (!name) return
  deleteTarget.value = null
  try {
    await invoke('delete_file', { name })
    toast(`已删除 ${name}`)
    await refreshFiles()
  } catch (e) {
    toast(`${e}`)
  }
}

async function sendText() {
  const content = textInput.value.trim()
  if (!content || sending.value) return
  sending.value = true
  try {
    await invoke('send_clip', { content })
    textInput.value = ''
    await refreshClips()
    toast('已发送到手机')
  } catch (e) {
    toast(`发送失败:${e}`)
  } finally {
    sending.value = false
  }
}

async function sendClipboardText() {
  if (sending.value) return
  sending.value = true
  try {
    const text = await readText()
    if (!text.trim()) {
      toast('剪贴板里没有文字')
      return
    }
    await invoke('send_clip', { content: text })
    await refreshClips()
    toast('剪贴板已发送')
  } catch (e) {
    toast(`读取剪贴板失败:${e}`)
  } finally {
    sending.value = false
  }
}

function toBase64(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf)
  let bin = ''
  const chunk = 0x8000
  for (let i = 0; i < bytes.length; i += chunk) {
    bin += String.fromCharCode(...bytes.subarray(i, i + chunk))
  }
  return btoa(bin)
}

async function onPaste(e: ClipboardEvent) {
  const items = e.clipboardData?.items
  if (!items) return
  for (const item of items) {
    if (!item.type.startsWith('image/')) continue
    e.preventDefault()
    const blob = item.getAsFile()
    if (!blob) continue
    try {
      const base64 = toBase64(await blob.arrayBuffer())
      const name = await invoke<string>('save_screenshot', { data: base64 })
      toast(`截图已分享:${name}`)
      await refreshFiles()
    } catch (err) {
      toast(`截图保存失败:${err}`)
    }
    return
  }
}

// ---- HTML5 拖放 ----
// 拖进窗口的文件走与手机相同的 /api/upload 通道,复用流式写盘、重名处理
let dragDepth = 0
let dragTimer: ReturnType<typeof setTimeout> | undefined

function isFileDrag(e: DragEvent): boolean {
  return Array.from(e.dataTransfer?.types ?? []).includes('Files')
}

function onDragEnter(e: DragEvent) {
  if (!isFileDrag(e)) return
  e.preventDefault()
  dragDepth++
  clearTimeout(dragTimer)
  dragging.value = true
}

function onDragOver(e: DragEvent) {
  if (!isFileDrag(e)) return
  e.preventDefault()
  // dragleave 在子元素间切换时频繁触发,用短延时防抖避免闪烁
  clearTimeout(dragTimer)
  dragTimer = setTimeout(() => {
    dragging.value = false
    dragDepth = 0
  }, 120)
}

function onDragLeave(e: DragEvent) {
  if (!isFileDrag(e)) return
  dragDepth = Math.max(0, dragDepth - 1)
  clearTimeout(dragTimer)
  dragTimer = setTimeout(() => {
    if (dragDepth === 0) dragging.value = false
  }, 120)
}

async function onDrop(e: DragEvent) {
  if (!isFileDrag(e)) return
  e.preventDefault()
  clearTimeout(dragTimer)
  dragDepth = 0
  dragging.value = false
  const dropped = Array.from(e.dataTransfer?.files ?? [])
  if (dropped.length === 0) return
  uploading.value = true
  let ok = 0
  try {
    for (const f of dropped) {
      const fd = new FormData()
      fd.append('file', f, f.name)
      const res = await fetch(uploadUrl.value, { method: 'POST', body: fd })
      if (res.ok) {
        ok++
      } else {
        toast(`${f.name} 上传失败(HTTP ${res.status})`)
      }
    }
    if (ok > 0) {
      toast(`已分享 ${ok} 个文件到手机`)
      await refreshFiles()
    }
  } catch (err) {
    toast(`上传失败:${err}`)
  } finally {
    uploading.value = false
  }
}

async function loadSettings() {
  try {
    const s = await invoke<Settings>('get_settings')
    settingsPort.value = s.port === 0 ? '' : String(s.port)
    settingsDir.value = s.inbox_dir ?? ''
    settingsLoaded.value = true
  } catch (e) {
    toast(`${e}`)
  }
}

async function chooseDir() {
  try {
    const picked = await invoke<string | null>('pick_inbox_dir')
    if (picked) settingsDir.value = picked
  } catch (e) {
    toast(`${e}`)
  }
}

async function saveSettings() {
  if (savingSettings.value) return
  const port = settingsPort.value.trim()
  if (port && (!/^\d+$/.test(port) || Number(port) > 65535)) {
    toast('端口需为 1–65535,留空则自动')
    return
  }
  savingSettings.value = true
  try {
    await invoke('save_settings', {
      port: port ? Number(port) : 0,
      inboxDir: settingsDir.value.trim() || null,
    })
    toast('设置已保存')
    await Promise.all([refreshFiles(), refreshClips()])
  } catch (e) {
    toast(`${e}`)
  } finally {
    savingSettings.value = false
  }
}

onMounted(async () => {
  // setup 里的服务可能稍晚于页面加载就绪,重试等待
  for (let i = 0; i < 20; i++) {
    try {
      info.value = await invoke<ServerInfo>('server_info')
      break
    } catch {
      await new Promise((r) => setTimeout(r, 250))
    }
  }
  if (info.value) {
    qr.value = await QRCode.toDataURL(info.value.url, { width: 260, margin: 1 })
    uploadUrl.value = `${info.value.url.replace(/\/$/, '')}/api/upload`
  } else {
    toast('服务启动失败,请查看控制台日志')
  }
  await Promise.all([refreshFiles(), refreshClips(), loadSettings()])

  unlisteners.push(
    await listen<{ name: string; size: number; source: string }>('file-added', async (event) => {
      await refreshFiles()
      // 手机发来的才弹提示;桌面自己的操作(拖拽/选择/截图)已有各自的 toast
      if (event.payload.source === 'phone' && !uploading.value) {
        toast(`收到 ${event.payload.name}(${fmtSize(event.payload.size)})`)
      }
    }),
  )

  unlisteners.push(
    await listen<Clip>('clip-added', async (event) => {
      await refreshClips()
      if (event.payload.source === 'phone') {
        toast('收到文字,已复制到剪贴板')
      }
    }),
  )

  // HTML5 拖放(不依赖 Tauri 原生 drag-drop 拦截,Windows WebView2 下更可靠)
  window.addEventListener('dragenter', onDragEnter)
  window.addEventListener('dragover', onDragOver)
  window.addEventListener('dragleave', onDragLeave)
  window.addEventListener('drop', onDrop)

  window.addEventListener('paste', onPaste)
})

onUnmounted(() => {
  unlisteners.forEach((u) => u())
  window.removeEventListener('dragenter', onDragEnter)
  window.removeEventListener('dragover', onDragOver)
  window.removeEventListener('dragleave', onDragLeave)
  window.removeEventListener('drop', onDrop)
  window.removeEventListener('paste', onPaste)
})
</script>

<template>
  <div class="page">
    <div class="shell">
      <header class="page-head">
        <div class="brand">
          <span class="brand-logo">
            <svg viewBox="0 0 24 24"><path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" /></svg>
          </span>
          <span class="brand-name">Pigeon</span>
        </div>
        <span v-if="info" class="status"><i></i>{{ info.host }}:{{ info.port }}</span>
      </header>

      <div class="card">
        <nav class="tabs">
          <button :class="{ on: tab === 'connect' }" @click="tab = 'connect'">连接</button>
          <button :class="{ on: tab === 'files' }" @click="tab = 'files'">
            文件<span v-if="files.length" class="tab-n">{{ files.length }}</span>
          </button>
          <button :class="{ on: tab === 'clips' }" @click="tab = 'clips'">记录</button>
          <button :class="{ on: tab === 'settings' }" @click="tab = 'settings'">设置</button>
        </nav>

        <!-- ========== 连接 ========== -->
        <div v-show="tab === 'connect'" class="body">
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
                  <span class="url">{{ info?.url ?? '…' }}</span>
                </div>
                <button class="sq-btn" title="复制链接" @click="copyUrl">
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
                placeholder="输入文字,回车发送"
                @keydown.enter.exact.prevent="sendText"
              />
              <button
                class="sq-btn wide"
                :disabled="!textInput.trim() || sending"
                @click="sendText"
              >
                发送
              </button>
            </div>
            <p class="sub">
              也可以
              <button class="link" @click="sendClipboardText">直接发送剪贴板内容</button>
            </p>
          </div>
        </div>

        <!-- ========== 文件 ========== -->
        <div v-show="tab === 'files'" class="body">
          <div class="drop" :class="{ over: dragging }" @click="browse">
            <div class="dz-icon">
              <svg viewBox="0 0 24 24">
                <path d="M9 16h6v-6h4l-7-7-7 7h4v6zm-4 2h14v2H5z" />
              </svg>
            </div>
            <p class="dz-t">{{ uploading ? '正在上传…' : '拖入文件或文件夹' }}</p>
            <p class="dz-s">拖进窗口、点击选择,或 Ctrl+V 粘贴截图 · 文件夹会自动打包成 zip</p>
          </div>

          <template v-if="files.length">
            <div class="list-head">
              <span class="sec">已共享</span>
              <span class="sec-n">{{ files.length }}</span>
            </div>
            <ul class="list">
              <li v-for="f in files" :key="f.name">
                <button class="row-main" :title="`在文件夹中显示 ${f.name}`" @click="reveal(f.name)">
                  <span class="ext">{{ extOf(f.name) }}</span>
                  <span class="row-text">
                    <span class="row-name">{{ f.name }}</span>
                    <span class="row-sub">{{ fmtSize(f.size) }} · {{ timeAgo(f.modified) }}</span>
                  </span>
                </button>
                <span class="row-ops">
                  <button class="op" title="复制下载链接" @click="copyFileUrl(f.name)">
                    <svg viewBox="0 0 24 24">
                      <path
                        fill="currentColor"
                        d="M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7a5 5 0 0 0 0 10h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4a5 5 0 0 0 0-10z"
                      />
                    </svg>
                  </button>
                  <button class="op danger" title="删除" @click="deleteTarget = f.name">
                    <svg viewBox="0 0 24 24">
                      <path
                        fill="currentColor"
                        d="M6 19a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                      />
                    </svg>
                  </button>
                </span>
              </li>
            </ul>
          </template>
          <p v-else class="empty">还没有文件,拖一个进来试试</p>

          <div class="foot-bar">
            <span class="path" :title="info?.inbox_dir">{{ info?.inbox_dir }}</span>
            <button class="link" @click="openFolder">打开文件夹</button>
          </div>
        </div>

        <!-- ========== 记录 ========== -->
        <div v-show="tab === 'clips'" class="body">
          <template v-if="clips.length">
            <div class="list-head">
              <span class="sec">文字记录</span>
              <span class="sec-n">{{ clips.length }}</span>
            </div>
            <ul class="list">
              <li v-for="c in clips" :key="c.id">
                <button class="row-main clip-row" title="点击复制" @click="copyText(c.content)">
                  <span class="row-text">
                    <span class="clip-text">{{ c.content }}</span>
                    <span class="row-sub">
                      {{ c.source === 'phone' ? '手机' : '电脑' }} · {{ fmtClipTime(c.created) }}
                    </span>
                  </span>
                  <span class="clip-copy">复制</span>
                </button>
              </li>
            </ul>
          </template>
          <p v-else class="empty">还没有文字记录,在「连接」里发一条试试</p>
        </div>

        <!-- ========== 设置 ========== -->
        <div v-show="tab === 'settings'" class="body">
          <div class="field">
            <label>监听端口</label>
            <input
              v-model="settingsPort"
              class="input"
              placeholder="留空自动分配(默认 8420)"
              inputmode="numeric"
            />
            <p class="sub">改动端口需重启应用生效;端口被占用时自动回落随机端口</p>
          </div>

          <div class="field">
            <label>共享目录</label>
            <div class="input-row">
              <input v-model="settingsDir" class="input" readonly placeholder="~/Pigeon/Inbox" />
              <button class="sq-btn wide" @click="chooseDir">选择…</button>
            </div>
            <p class="sub">修改后立即生效,已分享的文件留在原目录</p>
          </div>

          <div class="hr"></div>

          <div class="actions">
            <button
              class="btn primary"
              :disabled="savingSettings || !settingsLoaded"
              @click="saveSettings"
            >
              {{ savingSettings ? '保存中…' : '保存设置' }}
            </button>
          </div>

          <p class="sub note">
            设置保存在 <code>~/Pigeon/settings.json</code> · token 每次启动随机生成
          </p>
        </div>
      </div>
    </div>

    <div class="toasts">
      <TransitionGroup name="toast">
        <div v-for="t in toasts" :key="t.id" class="toast">{{ t.text }}</div>
      </TransitionGroup>
    </div>

    <div v-if="dragging" class="drag-overlay">
      <div class="drag-box">
        <p class="drag-t">松开,发送到手机</p>
        <p class="drag-s">文件夹将自动打包成 zip</p>
      </div>
    </div>

    <!-- 删除确认 -->
    <div v-if="deleteTarget" class="modal-mask" @click.self="deleteTarget = null">
      <div class="modal">
        <h2 class="modal-t">删除文件</h2>
        <p class="modal-desc">
          确定删除 <b>{{ deleteTarget }}</b> ?手机端将无法再下载该文件。
        </p>
        <div class="modal-actions">
          <button class="btn ghost" @click="deleteTarget = null">取消</button>
          <button class="btn primary danger" @click="doDelete">删除</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ========== 基础 ========== */
.page {
  min-height: 100vh;
  background: #f1f2f4;
  color: #1a1d24;
  font-family: system-ui, -apple-system, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  padding: 26px 16px 40px;
  overflow-y: auto;
  user-select: none;
}
.shell {
  max-width: 640px;
  margin: 0 auto;
}

/* 顶部品牌行 */
.page-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 4px 14px;
}
.brand {
  display: flex;
  align-items: center;
  gap: 9px;
}
.brand-logo {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  background: #1c6fd8;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.brand-logo svg {
  width: 14px;
  height: 14px;
  fill: #fff;
}
.brand-name {
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0.2px;
}
.status {
  font-size: 12px;
  color: #5b6478;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-family: ui-monospace, Consolas, monospace;
}
.status i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #22a06b;
}

/* 卡片 */
.card {
  background: #fff;
  border: 1px solid #e6e9ef;
  border-radius: 16px;
  overflow: hidden;
}

/* Tab */
.tabs {
  display: flex;
  gap: 26px;
  padding: 16px 22px 0;
  border-bottom: 1px solid #eef0f3;
}
.tabs button {
  position: relative;
  border: 0;
  background: none;
  padding: 0 0 13px;
  font-size: 14px;
  color: #8a93a8;
  cursor: pointer;
  font-family: inherit;
  transition: color 0.15s;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.tabs button:hover {
  color: #3b4557;
}
.tabs button.on {
  color: #1a1d24;
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
  background: #1c6fd8;
}
.tab-n {
  font-size: 11px;
  font-weight: 500;
  color: #8a93a8;
  background: #f2f4f7;
  border-radius: 999px;
  padding: 1px 7px;
  line-height: 1.6;
}

.body {
  padding: 22px;
}

/* ========== 通用元素 ========== */
.hr {
  height: 1px;
  background: #eef0f3;
  margin: 20px 0;
}

.field {
  margin-bottom: 18px;
}
.field label,
.sec {
  display: block;
  font-size: 13px;
  font-weight: 600;
  color: #1a1d24;
  margin-bottom: 8px;
}
.cap {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
}
.sub {
  font-size: 12px;
  color: #8a93a8;
  line-height: 1.6;
  margin-top: 7px;
}

.input {
  width: 100%;
  height: 40px;
  border: 1px solid #e6e9ef;
  background: #f7f8fa;
  border-radius: 9px;
  padding: 0 12px;
  font-size: 13px;
  font-family: inherit;
  color: #1a1d24;
  box-sizing: border-box;
  user-select: text;
  transition:
    border-color 0.15s,
    background 0.15s;
}
.input::placeholder {
  color: #a8b0c0;
}
.input:focus {
  outline: none;
  border-color: #1c6fd8;
  background: #fff;
}
.input.readonly,
.input[readonly] {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: text;
}
.input.readonly .lead {
  width: 15px;
  height: 15px;
  flex: none;
  color: #9aa3b5;
}
.url {
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
  color: #4a5468;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.input-row {
  display: flex;
  gap: 8px;
  align-items: stretch;
}
.input-row .input {
  flex: 1;
  min-width: 0;
}

.sq-btn {
  flex: none;
  width: 40px;
  border: 1px solid #e6e9ef;
  background: #fff;
  border-radius: 9px;
  color: #5b6478;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  font-family: inherit;
  transition:
    border-color 0.15s,
    background 0.15s,
    color 0.15s;
}
.sq-btn svg {
  width: 15px;
  height: 15px;
}
.sq-btn:hover:not(:disabled) {
  border-color: #cfd6e2;
  background: #f7f8fa;
  color: #1a1d24;
}
.sq-btn:active:not(:disabled) {
  transform: translateY(1px);
}
.sq-btn.wide {
  width: auto;
  min-width: 76px;
  padding: 0 16px;
  font-weight: 500;
}
.sq-btn:disabled {
  color: #b6bdcc;
  cursor: default;
}

.btn {
  border: 0;
  border-radius: 9px;
  padding: 0 18px;
  height: 40px;
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
}
.btn.primary {
  background: #1c6fd8;
  color: #fff;
  font-weight: 500;
}
.btn.primary:hover:not(:disabled) {
  background: #1a63c0;
}
.btn.primary:disabled {
  background: #b9cdf0;
  cursor: default;
}
.btn.ghost {
  background: #f2f4f7;
  color: #3b4557;
}
.btn.ghost:hover {
  background: #e9ecf1;
}
.btn.primary.danger {
  background: #e5484d;
}
.btn.primary.danger:hover {
  background: #d03a3f;
}
.btn:active:not(:disabled) {
  transform: translateY(1px);
}

.link {
  border: 0;
  background: none;
  padding: 0;
  color: #1c6fd8;
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
}
.link:hover {
  text-decoration: underline;
}

/* ========== 连接 ========== */
.qr-row {
  display: flex;
  gap: 20px;
  align-items: center;
}
.qr {
  width: 148px;
  height: 148px;
  border-radius: 10px;
  border: 1px solid #eef0f3;
  flex: none;
}
.qr-loading {
  background: linear-gradient(100deg, #f0f2f6 40%, #fafbfd 50%, #f0f2f6 60%);
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

/* ========== 拖放区 ========== */
.drop {
  border: 1.5px dashed #d4dae4;
  border-radius: 12px;
  padding: 22px 16px;
  text-align: center;
  cursor: pointer;
  transition:
    border-color 0.15s,
    background 0.15s;
  margin-bottom: 18px;
}
.drop:hover,
.drop.over {
  border-color: #1c6fd8;
  background: #f6f9ff;
}
.dz-icon {
  width: 38px;
  height: 38px;
  margin: 0 auto 9px;
  border-radius: 50%;
  background: #eef4ff;
  display: flex;
  align-items: center;
  justify-content: center;
}
.dz-icon svg {
  width: 19px;
  height: 19px;
  fill: #1c6fd8;
}
.dz-t {
  font-size: 13.5px;
  font-weight: 600;
}
.dz-s {
  font-size: 12px;
  color: #8a93a8;
  margin-top: 4px;
}

/* ========== 列表 ========== */
.list-head {
  display: flex;
  align-items: baseline;
  gap: 7px;
  margin: 4px 0 2px;
}
.list-head .sec {
  margin-bottom: 0;
}
.sec-n {
  font-size: 12px;
  color: #a2aab9;
}
.list {
  list-style: none;
  margin: 0;
  padding: 0;
}
.list li {
  display: flex;
  align-items: center;
  gap: 4px;
  border-bottom: 1px solid #f1f3f6;
}
.list li:last-child {
  border-bottom: 0;
}
.row-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 11px 2px;
  border: 0;
  background: none;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  border-radius: 8px;
  color: inherit;
}
.row-main:hover {
  background: #fafbfd;
}
.ext {
  flex: none;
  width: 36px;
  height: 36px;
  border-radius: 9px;
  background: #f2f4f8;
  color: #5b6478;
  font-size: 9px;
  font-weight: 700;
  text-transform: uppercase;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}
.row-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.row-name {
  font-size: 13px;
  color: #1a1d24;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.row-sub {
  font-size: 11.5px;
  color: #8a93a8;
}
.row-ops {
  flex: none;
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity 0.15s;
}
.list li:hover .row-ops {
  opacity: 1;
}
.op {
  border: 0;
  background: none;
  width: 28px;
  height: 28px;
  border-radius: 7px;
  color: #9aa3b5;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.op svg {
  width: 14px;
  height: 14px;
}
.op:hover {
  background: #eef4ff;
  color: #1c6fd8;
}
.op.danger:hover {
  background: #fdeeee;
  color: #e5484d;
}

.clip-row {
  align-items: flex-start;
  padding: 12px 2px;
}
.clip-text {
  font-size: 13px;
  line-height: 1.55;
  color: #1a1d24;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 3.3em;
  overflow: hidden;
  user-select: text;
}
.clip-copy {
  flex: none;
  align-self: center;
  font-size: 11.5px;
  color: #1c6fd8;
  background: #eef4ff;
  border-radius: 7px;
  padding: 5px 11px;
  opacity: 0;
  transition: opacity 0.15s;
}
.list li:hover .clip-copy {
  opacity: 1;
}

.empty {
  text-align: center;
  color: #a2aab9;
  font-size: 13px;
  padding: 26px 0 18px;
}

.foot-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 14px;
  padding-top: 12px;
  border-top: 1px solid #f1f3f6;
}
.path {
  font-size: 11px;
  font-family: ui-monospace, Consolas, monospace;
  color: #b0b8c9;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  user-select: text;
}

.actions {
  display: flex;
  justify-content: flex-end;
}
.note {
  margin-top: 16px;
}
.note code {
  font-family: ui-monospace, Consolas, monospace;
  background: #f2f4f7;
  border-radius: 4px;
  padding: 1px 5px;
  color: #5b6478;
}

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
  background: #1a1d24;
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

/* ========== 拖拽蒙层 ========== */
.drag-overlay {
  position: fixed;
  inset: 0;
  background: rgba(28, 111, 216, 0.07);
  z-index: 40;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.drag-box {
  background: #fff;
  border: 1.5px dashed #1c6fd8;
  border-radius: 14px;
  padding: 20px 32px;
  text-align: center;
}
.drag-t {
  font-size: 14px;
  font-weight: 700;
  color: #1c6fd8;
}
.drag-s {
  font-size: 12px;
  color: #8a93a8;
  margin-top: 4px;
}

/* ========== 弹窗 ========== */
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(26, 29, 36, 0.38);
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
}
.modal {
  background: #fff;
  border: 1px solid #e6e9ef;
  border-radius: 15px;
  padding: 22px;
  width: min(400px, 90vw);
}
.modal-t {
  font-size: 15px;
  margin-bottom: 10px;
}
.modal-desc {
  font-size: 13px;
  color: #5b6478;
  line-height: 1.6;
  margin-bottom: 4px;
  user-select: text;
}
.modal-desc b {
  color: #1a1d24;
  word-break: break-all;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 18px;
}
</style>
