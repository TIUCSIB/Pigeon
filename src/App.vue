<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import { getCurrentWindow } from '@tauri-apps/api/window'
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
// 确认弹窗类型:file=删单个文件 clips-clear=清空记录 files-clear=清空文件 files-batch=批量删除选中
type ConfirmKind = 'file' | 'clips-clear' | 'files-clear' | 'files-batch'
const confirmKind = ref<ConfirmKind | null>(null)
const deleteClipId = ref<number | null>(null)

// 文件多选模式
const selecting = ref(false)
const selected = ref<Set<string>>(new Set())

function toggleSelectMode() {
  selecting.value = !selecting.value
  selected.value = new Set()
}

function toggleSelected(name: string) {
  const next = new Set(selected.value)
  if (next.has(name)) {
    next.delete(name)
  } else {
    next.add(name)
  }
  selected.value = next
}

const allSelected = computed(
  () => files.value.length > 0 && selected.value.size === files.value.length,
)

function toggleSelectAll() {
  selected.value = allSelected.value ? new Set() : new Set(files.value.map((f) => f.name))
}

async function deleteSelected() {
  const names = Array.from(selected.value)
  confirmKind.value = null
  selecting.value = false
  selected.value = new Set()
  let ok = 0
  for (const name of names) {
    try {
      await invoke('delete_file', { name })
      ok++
    } catch (e) {
      toast(`${name}: ${e}`)
    }
  }
  if (ok > 0) {
    toast(`已删除 ${ok} 个文件`)
    await refreshFiles()
  }
}
// 桌面 UI 自己调 HTTP 上传接口(与手机端同通道)
const uploadUrl = ref('')

// 设置
const settingsPort = ref('')
const settingsDir = ref('')
const savingSettings = ref(false)
const settingsLoaded = ref(false)
// 暗黑模式(即时生效并持久化)
const darkMode = ref(false)
// 自绘标题栏(固定小卡片窗口,无最大化)
const appWindow = getCurrentWindow()

async function winMinimize() {
  await appWindow.minimize()
}
async function winClose() {
  await appWindow.close()
}

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
  if (confirmKind.value === 'file') {
    const name = deleteTarget.value
    if (!name) return
    deleteTarget.value = null
    confirmKind.value = null
    try {
      await invoke('delete_file', { name })
      toast(`已删除 ${name}`)
      await refreshFiles()
    } catch (e) {
      toast(`${e}`)
    }
    return
  }
  if (confirmKind.value === 'clips-clear') {
    confirmKind.value = null
    try {
      await invoke('clear_clips')
      toast('文字记录已清空')
      await refreshClips()
    } catch (e) {
      toast(`${e}`)
    }
    return
  }
  if (confirmKind.value === 'files-clear') {
    confirmKind.value = null
    try {
      await invoke('clear_files')
      toast('共享文件已清空')
      await refreshFiles()
    } catch (e) {
      toast(`${e}`)
    }
    return
  }
  if (confirmKind.value === 'files-batch') {
    await deleteSelected()
  }
}

async function deleteClip(c: Clip) {
  try {
    await invoke('delete_clip', { id: c.id })
    await refreshClips()
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

function applyTheme(dark: boolean) {
  darkMode.value = dark
  // 根元素类驱动 scoped 样式里的 .page.dark 变量;color-scheme 让滚动条等原生控件跟随
  document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
  // 同步 html 底色,窗口边缘露出的部分与画布一致(暗色防白边)
  document.documentElement.style.backgroundColor = dark ? '#14161a' : '#f1f2f4'
}

async function toggleDark() {
  const next = !darkMode.value
  applyTheme(next)
  try {
    // 同时切窗口原生主题(标题栏跟随)并持久化
    await invoke('set_theme', { dark: next })
  } catch (e) {
    toast(`${e}`)
  }
}

async function loadSettings() {
  try {
    const s = await invoke<Settings>('get_settings')
    settingsPort.value = s.port === 0 ? '' : String(s.port)
    settingsDir.value = s.inbox_dir ?? ''
    applyTheme(s.dark_mode)
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
  <div class="page" :class="{ dark: darkMode }">
    <div class="shell">
      <!-- 自绘标题栏:整行可拖拽,右侧窗口控制按钮 -->
      <header class="titlebar" data-tauri-drag-region>
        <div class="brand" data-tauri-drag-region>
          <span class="brand-logo" data-tauri-drag-region>
            <svg viewBox="0 0 24 24" data-tauri-drag-region><path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" /></svg>
          </span>
          <span class="brand-name" data-tauri-drag-region>Pigeon</span>
        </div>
        <div class="titlebar-right">
          <span v-if="info" class="status" data-tauri-drag-region><i></i>{{ info.host }}:{{ info.port }}</span>
          <div class="win-btns">
            <button class="win-btn" title="最小化" @click="winMinimize">
              <svg viewBox="0 0 10 10"><path d="M0 5h10" stroke="currentColor" stroke-width="1" /></svg>
            </button>
            <button class="win-btn close" title="关闭" @click="winClose">
              <svg viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" /></svg>
            </button>
          </div>
        </div>
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
        <div v-show="tab === 'files'" class="body files-body">
          <div class="body-scroll">
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
                <button v-if="!selecting" class="link danger-link" @click="confirmKind = 'files-clear'">
                  清空
                </button>
                <button class="link" @click="toggleSelectMode">
                  {{ selecting ? '取消选择' : '选择' }}
                </button>
              </div>
              <ul class="list">
                <li v-for="f in files" :key="f.name">
                  <!-- 多选模式:行首勾选框,点行切换选中 -->
                  <button
                    v-if="selecting"
                    class="check"
                    :class="{ on: selected.has(f.name) }"
                    @click="toggleSelected(f.name)"
                  >
                    <svg v-if="selected.has(f.name)" viewBox="0 0 24 24">
                      <path
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        d="M20 6L9 17l-5-5"
                      />
                    </svg>
                  </button>
                  <button
                    class="row-main"
                    :class="{ dim: selecting }"
                    :title="selecting ? '' : `在文件夹中显示 ${f.name}`"
                    @click="selecting ? toggleSelected(f.name) : reveal(f.name)"
                  >
                    <span class="ext">{{ extOf(f.name) }}</span>
                    <span class="row-text">
                      <span class="row-name">{{ f.name }}</span>
                      <span class="row-sub">{{ fmtSize(f.size) }} · {{ timeAgo(f.modified) }}</span>
                    </span>
                  </button>
                  <span v-if="!selecting" class="row-ops">
                    <button class="op" title="复制下载链接" @click="copyFileUrl(f.name)">
                      <svg viewBox="0 0 24 24">
                        <path
                          fill="currentColor"
                          d="M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7a5 5 0 0 0 0 10h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4a5 5 0 0 0 0-10z"
                        />
                      </svg>
                    </button>
                    <button class="op danger" title="删除" @click="deleteTarget = f.name; confirmKind = 'file'">
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
              <!-- 多选模式下的全选行 -->
              <div v-if="selecting" class="select-all-row">
                <button class="check" :class="{ on: allSelected }" @click="toggleSelectAll">
                  <svg v-if="allSelected" viewBox="0 0 24 24">
                    <path
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      d="M20 6L9 17l-5-5"
                    />
                  </svg>
                </button>
                <span class="sa-t">全选</span>
              </div>
            </template>
            <p v-else class="empty">还没有文件,拖一个进来试试</p>
          </div>

          <!-- 固定底栏:多选时显示批量操作,否则显示路径 -->
          <div v-if="selecting" class="dock-bar">
            <span class="batch-n">已选 {{ selected.size }} 项</span>
            <button
              class="btn primary danger"
              :disabled="selected.size === 0"
              @click="confirmKind = 'files-batch'"
            >
              删除选中
            </button>
          </div>
          <div v-else class="dock-bar foot-bar">
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
              <button class="link danger-link" @click="confirmKind = 'clips-clear'">清空</button>
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
                <span class="row-ops">
                  <button class="op danger" title="删除" @click="deleteClip(c)">
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
          <p v-else class="empty">还没有文字记录,在「连接」里发一条试试</p>
        </div>

        <!-- ========== 设置 ========== -->
        <div v-show="tab === 'settings'" class="body">
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
              @click="toggleDark"
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

    <!-- 删除/清空确认 -->
    <div v-if="confirmKind" class="modal-mask" @click.self="confirmKind = null">
      <div class="modal">
        <h2 class="modal-t">
          {{
            confirmKind === 'file'
              ? '删除文件'
              : confirmKind === 'clips-clear'
                ? '清空文字记录'
                : confirmKind === 'files-batch'
                  ? '删除选中文件'
                  : '清空共享文件'
          }}
        </h2>
        <p v-if="confirmKind === 'file'" class="modal-desc">
          确定删除 <b>{{ deleteTarget }}</b> ?手机端将无法再下载该文件。
        </p>
        <p v-else-if="confirmKind === 'clips-clear'" class="modal-desc">
          确定清空全部 {{ clips.length }} 条文字记录?此操作不可恢复。
        </p>
        <p v-else-if="confirmKind === 'files-batch'" class="modal-desc">
          确定删除选中的 {{ selected.size }} 个文件?此操作不可恢复。
        </p>
        <p v-else class="modal-desc">
          确定清空共享目录中的 {{ files.length }} 个文件?手机端将无法再下载,此操作不可恢复。
        </p>
        <div class="modal-actions">
          <button class="btn ghost" @click="confirmKind = null">取消</button>
          <button class="btn primary danger" @click="doDelete">删除</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 细滚动条,契合扁平主题;颜色走变量,暗色自动跟随 */
.page ::-webkit-scrollbar,
html::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}
.page ::-webkit-scrollbar-track,
html::-webkit-scrollbar-track {
  background: transparent;
}
.page ::-webkit-scrollbar-thumb,
html::-webkit-scrollbar-thumb {
  background: var(--border);
  border-radius: 999px;
}
.page ::-webkit-scrollbar-thumb:hover,
html::-webkit-scrollbar-thumb:hover {
  background: var(--text-4);
}
.page ::-webkit-scrollbar-corner,
html::-webkit-scrollbar-corner {
  background: transparent;
}

/* ========== 基础 ========== */
.page {
  /* 亮色主题变量(暗色覆盖见 .dark) */
  --bg: #f1f2f4;
  --card: #fff;
  --border: #e6e9ef;
  --border-soft: #eef0f3;
  --border-row: #f1f3f6;
  --text: #1a1d24;
  --text-2: #5b6478;
  --text-3: #8a93a8;
  --text-4: #a8b0c0;
  --text-mono: #4a5468;
  --hover: #fafbfd;
  --field-bg: #f7f8fa;
  --chip-bg: #f2f4f7;
  --chip-n: #f2f4f7;
  --accent: #1c6fd8;
  --accent-hover: #1a63c0;
  --accent-bg: #eef4ff;
  --accent-btn-hover: #f7f8fa;
  --danger: #e5484d;
  --danger-bg: #fdeeee;
  --ok: #22a06b;
  --mask: rgba(26, 29, 36, 0.38);
  --toast-bg: #1a1d24;
  --drag-tint: rgba(28, 111, 216, 0.07);
  height: 100vh;
  box-sizing: border-box; /* 底部 padding 要算进 100vh,否则会被裁到窗口外 */
  /* 画布底色,内容卡片浮在其上 */
  background: var(--bg);
  color: var(--text);
  font-family: system-ui, -apple-system, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  padding: 0 8px 10px;
  overflow: hidden;
  user-select: none;
  display: flex;
  flex-direction: column;
}
.page.dark {
  --bg: #14161a;
  --card: #1c1f24;
  --border: #2e323a;
  --border-soft: #262a31;
  --border-row: #262a31;
  --text: #e8eaee;
  --text-2: #9aa3b5;
  --text-3: #737d92;
  --text-4: #5b6478;
  --text-mono: #a8b0c0;
  --hover: #22262d;
  --field-bg: #22262d;
  --chip-bg: #22262d;
  --chip-n: #22262d;
  --accent: #4d8ff0;
  --accent-hover: #67a2f3;
  --accent-bg: #1d2a3d;
  --accent-btn-hover: #22262d;
  --danger: #ef6b6f;
  --danger-bg: #3a2224;
  --ok: #35c287;
  --mask: rgba(0, 0, 0, 0.55);
  --toast-bg: #2e323a;
  --drag-tint: rgba(77, 143, 240, 0.09);
}
.shell {
  max-width: none;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* 自绘标题栏:通到窗口边缘,整行可拖拽 */
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 46px;
  margin: 0 -16px; /* 抵消页面左右 padding */
  padding-left: 16px;
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
  color: var(--text);
}
.status {
  font-size: 12px;
  color: var(--text-2);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-family: ui-monospace, Consolas, monospace;
}
.status i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ok);
}

/* 内容卡:独立浮层,圆角 + 描边,与画布区分 */
.card {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 16px;
  overflow: hidden;
  margin-top: 6px;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

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

.body {
  padding: 22px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* 文件页:内部滚动区 + 固定底栏(批量操作/路径),底栏不参与滚动 */
.files-body {
  display: flex;
  flex-direction: column;
  padding: 0;
}
.body-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 22px 22px 12px;
}
.dock-bar {
  flex: none;
  padding: 12px 22px;
  border-top: 1px solid var(--border-soft);
  background: var(--card);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

/* ========== 通用元素 ========== */
.hr {
  height: 1px;
  background: var(--border-soft);
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
  color: var(--text);
  margin-bottom: 8px;
}
.cap {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--text);
}
.sub {
  font-size: 12px;
  color: var(--text-3);
  line-height: 1.6;
  margin-top: 7px;
}

.input {
  width: 100%;
  height: 40px;
  border: 1px solid var(--border);
  background: var(--field-bg);
  border-radius: 9px;
  padding: 0 12px;
  font-size: 13px;
  font-family: inherit;
  color: var(--text);
  box-sizing: border-box;
  user-select: text;
  transition:
    border-color 0.15s,
    background 0.15s;
}
.input::placeholder {
  color: var(--text-4);
}
.input:focus {
  outline: none;
  border-color: var(--accent);
  background: var(--card);
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
  color: var(--text-4);
}
.url {
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
  color: var(--text-mono);
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
  border: 1px solid var(--border);
  background: var(--card);
  border-radius: 9px;
  color: var(--text-2);
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
  border-color: var(--border);
  background: var(--accent-btn-hover);
  color: var(--text);
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
  color: var(--text-4);
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
  background: var(--accent);
  color: #fff;
  font-weight: 500;
}
.btn.primary:hover:not(:disabled) {
  background: var(--accent-hover);
}
.btn.primary:disabled {
  background: var(--accent-bg);
  cursor: default;
}
.btn.ghost {
  background: var(--chip-bg);
  color: var(--text-2);
}
.btn.ghost:hover {
  background: var(--hover);
}
.btn.primary.danger {
  background: var(--danger);
}
.btn.primary.danger:hover {
  background: var(--danger);
  opacity: 0.9;
}
.btn:active:not(:disabled) {
  transform: translateY(1px);
}

.link {
  border: 0;
  background: none;
  padding: 0;
  color: var(--accent);
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

/* ========== 拖放区 ========== */
.drop {
  border: 1.5px dashed var(--border);
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
  border-color: var(--accent);
  background: var(--accent-bg);
}
.dz-icon {
  width: 38px;
  height: 38px;
  margin: 0 auto 9px;
  border-radius: 50%;
  background: var(--accent-bg);
  display: flex;
  align-items: center;
  justify-content: center;
}
.dz-icon svg {
  width: 19px;
  height: 19px;
  fill: var(--accent);
}
.dz-t {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text);
}
.dz-s {
  font-size: 12px;
  color: var(--text-3);
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
.list-head .danger-link {
  margin-left: auto;
}
.danger-link {
  color: var(--danger);
}
.sec-n {
  font-size: 12px;
  color: var(--text-4);
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
  border-bottom: 1px solid var(--border-row);
  /* 行与行留出呼吸感(选中高亮时相邻行不至于粘连) */
  padding: 4px 0;
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
  padding: 10px 10px;
  border: 0;
  background: none;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  border-radius: 8px;
  color: inherit;
}
.row-main:hover {
  background: var(--hover);
}
/* 多选模式下行淡化,点行即勾选 */
.row-main.dim {
  opacity: 0.75;
}
li:has(.check.on) .row-main {
  background: var(--accent-bg);
}

/* 多选勾选框 */
.check {
  flex: none;
  width: 20px;
  height: 20px;
  border: 1.5px solid var(--border);
  border-radius: 6px;
  background: var(--card);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  padding: 0;
  transition:
    background 0.12s,
    border-color 0.12s;
}
.check svg {
  width: 12px;
  height: 12px;
}
.check:hover {
  border-color: var(--accent);
}
.check.on {
  background: var(--accent);
  border-color: var(--accent);
}

/* 全选行 */
.select-all-row {
  display: flex;
  align-items: center;
  gap: 8px;
  /* 左缘与列表行勾选框对齐(li 无水平 padding,check 是行内第一个元素) */
  padding: 12px 10px 6px 0;
}
.sa-t {
  font-size: 12px;
  color: var(--text-3);
}

.batch-n {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.dock-bar .btn.primary.danger:disabled {
  background: var(--accent-bg);
  color: var(--text-4);
}
.ext {
  flex: none;
  width: 36px;
  height: 36px;
  border-radius: 9px;
  background: var(--chip-bg);
  color: var(--text-2);
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
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.row-sub {
  font-size: 11.5px;
  color: var(--text-3);
}
.row-ops {
  flex: none;
  display: flex;
  gap: 2px;
  padding-right: 4px;
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
  color: var(--text-3);
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
  background: var(--accent-bg);
  color: var(--accent);
}
.op.danger:hover {
  background: var(--danger-bg);
  color: var(--danger);
}

.clip-row {
  align-items: flex-start;
  padding: 12px 10px;
}
.clip-text {
  font-size: 13px;
  line-height: 1.55;
  color: var(--text);
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
  color: var(--accent);
  background: var(--accent-bg);
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
  color: var(--text-4);
  font-size: 13px;
  padding: 26px 0 18px;
}

.foot-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.path {
  font-size: 11px;
  font-family: ui-monospace, Consolas, monospace;
  color: var(--text-4);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  user-select: text;
}

.actions {
  display: flex;
  justify-content: flex-end;
}

/* 设置行(图标 + 文案 + 开关) */
.opt-row {
  display: flex;
  align-items: center;
  gap: 12px;
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
.opt-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.opt-t {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.opt-s {
  font-size: 12px;
  color: var(--text-3);
}
.switch {
  flex: none;
  width: 42px;
  height: 24px;
  border: 0;
  border-radius: 999px;
  background: var(--border);
  cursor: pointer;
  position: relative;
  padding: 0;
  transition: background 0.18s;
}
.switch .knob {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fff;
  transition: left 0.18s;
  box-shadow: none;
}
.switch.on {
  background: var(--accent);
}
.switch.on .knob {
  left: 21px;
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

/* ========== 拖拽蒙层 ========== */
.drag-overlay {
  position: fixed;
  inset: 0;
  background: var(--drag-tint);
  z-index: 40;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.drag-box {
  background: var(--card);
  border: 1.5px dashed #1c6fd8;
  border-radius: 14px;
  padding: 20px 32px;
  text-align: center;
}
.drag-t {
  font-size: 14px;
  font-weight: 700;
  color: var(--accent);
}
.drag-s {
  font-size: 12px;
  color: var(--text-3);
  margin-top: 4px;
}

/* ========== 弹窗 ========== */
.modal-mask {
  position: fixed;
  inset: 0;
  background: var(--mask);
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
}
.modal {
  background: var(--card);
  border: 1px solid #e6e9ef;
  border-radius: 15px;
  padding: 22px;
  width: min(400px, 90vw);
}
.modal-t {
  font-size: 15px;
  margin-bottom: 10px;
  color: var(--text);
}
.modal-desc {
  font-size: 13px;
  color: var(--text-2);
  line-height: 1.6;
  margin-bottom: 4px;
  user-select: text;
}
.modal-desc b {
  color: var(--text);
  word-break: break-all;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 18px;
}
</style>
