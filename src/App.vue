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
  sharing?: boolean
  dark_mode?: boolean
  token_enabled?: boolean
  auth_enabled?: boolean
  max_concurrent_uploads?: number
  speed_limit_mbps?: number
}

interface AccountView {
  username: string
  allow_download: boolean
  allow_upload_file: boolean
  allow_upload_text: boolean
  allow_delete: boolean
}

interface FrozenEntry {
  name: string
  size: number
}

type Tab = 'connect' | 'files' | 'clips' | 'security' | 'settings'

const tab = ref<Tab>('connect')
const info = ref<ServerInfo | null>(null)
const qr = ref('')
const files = ref<FileEntry[]>([])
const clips = ref<Clip[]>([])

// 文件/记录:搜索 + 排序(纯前端过滤,数据量大时也即时响应)
const fileQuery = ref('')
const fileSort = ref<'time' | 'name' | 'size'>('time')
const fileSortAsc = ref(false)
const clipQuery = ref('')
const clipSort = ref<'time' | 'name'>('time')

const filteredFiles = computed(() => {
  const q = fileQuery.value.trim().toLowerCase()
  let list = q
    ? files.value.filter((f) => f.name.toLowerCase().includes(q))
    : [...files.value]
  const dir = fileSortAsc.value ? 1 : -1
  list.sort((a, b) => {
    if (fileSort.value === 'name') return a.name.localeCompare(b.name, 'zh-CN') * dir
    if (fileSort.value === 'size') return (a.size - b.size) * dir
    return (a.modified - b.modified) * dir
  })
  return list
})

const filteredClips = computed(() => {
  const q = clipQuery.value.trim().toLowerCase()
  let list = q
    ? clips.value.filter((c) => c.content.toLowerCase().includes(q))
    : [...clips.value]
  const dir = clipSort.value === 'name' ? 1 : -1 // 记录默认最新在前
  if (clipSort.value === 'name') {
    list.sort((a, b) => a.content.localeCompare(b.content, 'zh-CN') * dir)
  } else {
    list.sort((a, b) => (a.created - b.created) * dir)
  }
  return list
})

function cycleFileSort() {
  const order: Array<'time' | 'name' | 'size'> = ['time', 'name', 'size']
  const i = order.indexOf(fileSort.value)
  fileSort.value = order[(i + 1) % order.length]
}
function cycleClipSort() {
  clipSort.value = clipSort.value === 'time' ? 'name' : 'time'
}

const fileSortLabel = computed(
  () => ({ time: '按时间', name: '按名称', size: '按大小' })[fileSort.value],
)
const fileSortDirLabel = computed(() => (fileSortAsc.value ? '↑' : '↓'))
const clipSortLabel = computed(() => (clipSort.value === 'time' ? '按时间' : '按名称'))

// 命中片段高亮:按查询词(不区分大小写)把文本切成 普通/命中 交替段,模板循环渲染
function highlight(text: string, query: string): Array<{ t: string; hit: boolean }> {
  const q = query.trim()
  if (!q) return [{ t: text, hit: false }]
  const lower = text.toLowerCase()
  const ql = q.toLowerCase()
  const parts: Array<{ t: string; hit: boolean }> = []
  let i = 0
  while (i < text.length) {
    const idx = lower.indexOf(ql, i)
    if (idx < 0) {
      parts.push({ t: text.slice(i), hit: false })
      break
    }
    if (idx > i) parts.push({ t: text.slice(i, idx), hit: false })
    parts.push({ t: text.slice(idx, idx + q.length), hit: true })
    i = idx + q.length
  }
  return parts
}
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
const settingsLoaded = ref(false)
const maxConcurrent = ref('')
const speedLimit = ref('')
// 暗黑模式(即时生效并持久化)
const darkMode = ref(false)
// 分享开关(绿点呼吸/红点)
const sharing = ref(true)
// 安全:token / 账号认证
const tokenEnabled = ref(true)
const authEnabled = ref(false)
const accounts = ref<AccountView[]>([])
const newAccUser = ref('')
const newAccPass = ref('')
const newAccDl = ref(true)
const newAccUf = ref(true)
const newAccUt = ref(true)
const newAccDel = ref(false)
const showAccModal = ref(false)
// 冻结文件列表(多选模式或折叠面板展示)
const frozen = ref<FrozenEntry[]>([])
const showFrozen = ref(false)
// 自绘标题栏(固定小卡片窗口,无最大化)
const appWindow = getCurrentWindow()

async function winMinimize() {
  await appWindow.minimize()
}
async function winClose() {
  await appWindow.close()
}

async function toggleSharing() {
  const next = !sharing.value
  try {
    await invoke('toggle_sharing', { on: next })
    sharing.value = next
    toast(next ? '分享已开启' : '分享已关闭,手机端将无法访问')
  } catch (e) {
    toast(`${e}`)
  }
}

async function refreshFrozen() {
  try {
    frozen.value = await invoke<FrozenEntry[]>('list_frozen')
  } catch {
    // ignore
  }
}

async function freezeFile(name: string) {
  try {
    await invoke('freeze_file', { name })
    toast(`已取消分享 ${name}(源文件保留)`)
    await Promise.all([refreshFiles(), refreshFrozen()])
  } catch (e) {
    toast(`${e}`)
  }
}

async function unfreezeFile(name: string) {
  try {
    await invoke('unfreeze_file', { name })
    toast(`已恢复分享 ${name}`)
    await Promise.all([refreshFiles(), refreshFrozen()])
  } catch (e) {
    toast(`${e}`)
  }
}

async function deleteFrozen(name: string) {
  try {
    await invoke('delete_frozen', { name })
    toast('已彻底删除')
    await refreshFrozen()
  } catch (e) {
    toast(`${e}`)
  }
}

// 连接排查
const showHelp = ref(false)

async function openFirewall() {
  try {
    await invoke('open_firewall_settings')
  } catch (e) {
    toast(`${e}`)
  }
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

/// 选择文件夹分享(后端自动打包成 zip)
async function browseFolder() {
  const picked = await open({ directory: true, multiple: true })
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
    toast('发送成功')
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

  // 区分文件/文件夹:文件夹不能走 HTTP 上传(会被当成 0 字节文件),
  // 提示用户用"选择文件夹"(走后端打包 zip)
  const items = Array.from(e.dataTransfer?.items ?? [])
  let skippedDirs = 0
  const files: File[] = []
  for (const it of items) {
    if (it.kind !== 'file') continue
    const entry = (it as DataTransferItem & {
      webkitGetAsEntry?: () => FileSystemEntry | null
    }).webkitGetAsEntry?.()
    if (entry?.isDirectory) {
      skippedDirs++
      continue
    }
    const f = it.getAsFile()
    if (f) files.push(f)
  }
  if (files.length === 0 && skippedDirs > 0) {
    toast('文件夹请点「选择文件夹」,会自动打包成 zip')
    return
  }
  if (skippedDirs > 0) {
    toast(`已跳过 ${skippedDirs} 个文件夹,文件夹请用「选择文件夹」`)
  }
  if (files.length === 0) return

  uploading.value = true
  let ok = 0
  try {
    for (const f of files) {
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
    maxConcurrent.value = String(s.max_concurrent_uploads ?? 2)
    speedLimit.value = String(s.speed_limit_mbps ?? 0)
    applyTheme(s.dark_mode === true)
    sharing.value = s.sharing !== false
    tokenEnabled.value = s.token_enabled !== false
    authEnabled.value = s.auth_enabled === true
    settingsLoaded.value = true
  } catch (e) {
    toast(`${e}`)
  }
}

async function refreshAccounts() {
  try {
    accounts.value = await invoke<AccountView[]>('list_accounts')
  } catch {
    // ignore
  }
}

async function refreshServerInfo() {
  try {
    info.value = await invoke<ServerInfo>('server_info')
    qr.value = await QRCode.toDataURL(info.value.url, { width: 260, margin: 1 })
  } catch {
    // ignore
  }
}

async function toggleToken() {
  const next = !tokenEnabled.value
  try {
    await invoke('toggle_token', { on: next })
    tokenEnabled.value = next
    await refreshServerInfo()
    toast(next ? '已启用链接 token' : '已关闭 token,局域网内任何人可直接访问')
  } catch (e) {
    toast(`${e}`)
  }
}

async function toggleAuth() {
  const next = !authEnabled.value
  try {
    await invoke('toggle_auth', { on: next })
    authEnabled.value = next
    toast(next ? '已开启账号认证,手机端需登录' : '已关闭账号认证')
  } catch (e) {
    toast(`${e}`)
  }
}

async function addAccount() {
  if (!newAccUser.value.trim() || !newAccPass.value) {
    toast('用户名和密码不能为空')
    return
  }
  try {
    await invoke('add_account', {
      username: newAccUser.value.trim(),
      password: newAccPass.value,
      allowDownload: newAccDl.value,
      allowUploadFile: newAccUf.value,
      allowUploadText: newAccUt.value,
      allowDelete: newAccDel.value,
    })
    newAccUser.value = ''
    newAccPass.value = ''
    newAccDl.value = true
    newAccUf.value = true
    newAccUt.value = true
    newAccDel.value = false
    showAccModal.value = false
    await refreshAccounts()
    toast('账号已添加')
  } catch (e) {
    toast(`${e}`)
  }
}

function openAccModal() {
  newAccUser.value = ''
  newAccPass.value = ''
  newAccDl.value = true
  newAccUf.value = true
  newAccUt.value = true
  newAccDel.value = false
  showAccModal.value = true
}

async function removeAccount(username: string) {
  try {
    await invoke('remove_account', { username })
    await refreshAccounts()
    // 最后一个账号删除后后端会自动关认证
    await loadSettings()
    toast(`已删除账号 ${username}`)
  } catch (e) {
    toast(`${e}`)
  }
}

async function chooseDir() {
  try {
    const picked = await invoke<string | null>('pick_inbox_dir')
    if (picked) {
      settingsDir.value = picked
      // 即改即存
      await persistSettings()
    }
  } catch (e) {
    toast(`${e}`)
  }
}

// 即改即存:端口/目录任一变化后 600ms 防抖保存
let saveTimer: ReturnType<typeof setTimeout> | undefined

function onSettingInput() {
  clearTimeout(saveTimer)
  saveTimer = setTimeout(persistSettings, 600)
}

async function persistSettings() {
  const port = settingsPort.value.trim()
  if (port && (!/^\d+$/.test(port) || Number(port) > 65535)) {
    return // 无效输入不保存,提示由校验分支给
  }
  try {
    await invoke('save_settings', {
      port: port ? Number(port) : 0,
      inboxDir: settingsDir.value.trim() || null,
      maxConcurrentUploads: Number(maxConcurrent.value) || 2,
      speedLimitMbps: Number(speedLimit.value) || 0,
    })
    toast('设置已保存')
    await Promise.all([refreshFiles(), refreshClips()])
  } catch (e) {
    toast(`${e}`)
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
  await Promise.all([
    refreshFiles(),
    refreshClips(),
    loadSettings(),
    refreshFrozen(),
    refreshAccounts(),
  ])

  unlisteners.push(
    await listen<boolean>('sharing-changed', (event) => {
      sharing.value = event.payload
    }),
  )

  // 托盘菜单"复制连接地址":复制逻辑在前端,复用现有 copyUrl
  // 事件名须与 src-tauri/src/tray.rs 的 COPY_URL_EVENT 一致
  unlisteners.push(await listen('tray-copy-url', () => void copyUrl()))

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
            <img src="/logo.png" alt="Pigeon" data-tauri-drag-region draggable="false" />
          </span>
          <span class="brand-name" data-tauri-drag-region>Pigeon</span>
        </div>
        <div class="titlebar-right">
          <button
            class="status"
            :class="{ off: !sharing }"
            :title="sharing ? '分享开启中,点击关闭' : '分享已关闭,点击开启'"
            @click="toggleSharing"
          >
            <i></i>{{ sharing ? '分享中' : '已关闭' }}
          </button>
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
          <button :class="{ on: tab === 'security' }" @click="tab = 'security'">安全</button>
          <button :class="{ on: tab === 'settings' }" @click="tab = 'settings'">设置</button>
        </nav>

        <!-- ========== 连接 ========== -->
        <div v-show="tab === 'connect'" class="body">
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
                    <span class="url">{{ info?.url ?? '…' }}</span>
                  </div>
                  <button
                    class="sq-btn"
                    title="复制链接"
                    :disabled="!sharing"
                    @click="copyUrl"
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
                  @keydown.enter.exact.prevent="sendText"
                />
                <button
                  class="sq-btn wide"
                  :disabled="!textInput.trim() || sending || !sharing"
                  @click="sendText"
                >
                  发送
                </button>
              </div>
              <p class="sub">
                也可以
                <button class="link" :disabled="!sharing" @click="sendClipboardText">
                  直接发送剪贴板内容
                </button>
              </p>
            </div>

            <!-- 关闭时的遮罩:灰化内容 + 居中开启入口 -->
            <div v-if="!sharing" class="off-overlay">
              <p class="off-t">分享已关闭</p>
              <p class="off-s">手机端无法访问,二维码与链接已失效</p>
              <button class="btn primary" @click="toggleSharing">开启分享</button>
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
            <button class="btn ghost" @click="openFirewall">打开防火墙设置</button>
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
              <p class="dz-s">拖进窗口、点击选择,或 Ctrl+V 粘贴截图</p>
              <div class="dz-actions">
                <button class="dz-btn" @click.stop="browse">选择文件</button>
                <button class="dz-btn" @click.stop="browseFolder">选择文件夹</button>
              </div>
              <p class="dz-s">文件夹会打包成 zip 分享</p>
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
              <div class="tools-row">
                <input
                  v-model="fileQuery"
                  class="tools-search"
                  type="search"
                  placeholder="搜索文件名…"
                />
                <button
                  class="sort-btn"
                  :title="`当前${fileSortLabel}${fileSortDirLabel},点击切换排序字段`"
                  @click="cycleFileSort"
                >
                  {{ fileSortLabel }} {{ fileSortDirLabel }}
                </button>
              </div>
              <ul class="list">
                <li v-for="f in filteredFiles" :key="f.name">
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
                      <span class="row-name"
                        ><template
                          v-for="(seg, si) in highlight(f.name, fileQuery)"
                          :key="si"
                          ><mark v-if="seg.hit" class="hl">{{ seg.t }}</mark
                          ><template v-else>{{ seg.t }}</template></template
                        ></span
                      >
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
                    <button class="op" title="取消分享(源文件保留)" @click="freezeFile(f.name)">
                      <svg viewBox="0 0 24 24">
                        <path
                          fill="currentColor"
                          d="M20.3 17.6c-.3-.6-.9-1-1.5-1.2.4-.6.6-1.3.5-2.1-.2-1.5-1.5-2.7-3-2.9-.7-.1-1.4 0-2 .3C14 10 12.7 8.9 11.1 8.9c-1.9 0-3.5 1.4-3.7 3.3-1.4.2-2.5 1.3-2.7 2.7-.2 1.6.8 3.1 2.3 3.5.3.1.6.1.9.1h10.5c.6 0 1.2-.3 1.7-.7.3-.1.3-.2.2-.2zM12 5.5l2.5 2.5H13v3h-2V8H9.5L12 5.5zM19 14.5l-2.5-2.5H17V9h-2v3h-1.5L16 14.5 19 14.5z"
                          transform="scale(0.9) translate(1.5 1.5)"
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

            <!-- 已冻结(取消分享)的文件:折叠面板 -->
            <template v-if="frozen.length">
              <button class="frozen-toggle" @click="showFrozen = !showFrozen">
                <svg
                  viewBox="0 0 24 24"
                  class="chev"
                  :class="{ open: showFrozen }"
                >
                  <path fill="currentColor" d="M7.4 8.6L12 13.2l4.6-4.6L18 10l-6 6-6-6z" />
                </svg>
                已取消分享 <span class="sec-n">{{ frozen.length }}</span>
              </button>
              <ul v-show="showFrozen" class="list frozen-list">
                <li v-for="fr in frozen" :key="fr.name">
                  <button class="row-main" :title="fr.name">
                    <span class="ext frozen-ext">❄</span>
                    <span class="row-text">
                      <span class="row-name">{{ fr.name }}</span>
                      <span class="row-sub">{{ fmtSize(fr.size) }} · 手机端不可见</span>
                    </span>
                  </button>
                  <span class="row-ops">
                    <button class="op" title="恢复分享" @click="unfreezeFile(fr.name)">
                      <svg viewBox="0 0 24 24">
                        <path
                          fill="currentColor"
                          d="M9 16h6v-6h4l-7-7-7 7h4v6zm-4 2h14v2H5z"
                        />
                      </svg>
                    </button>
                    <button class="op danger" title="彻底删除" @click="deleteFrozen(fr.name)">
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
            <div class="tools-row">
              <input
                v-model="clipQuery"
                class="tools-search"
                type="search"
                placeholder="搜索记录内容…"
              />
              <button class="sort-btn" title="切换排序方式" @click="cycleClipSort">
                {{ clipSortLabel }}
              </button>
            </div>
            <ul class="list">
              <li v-for="c in filteredClips" :key="c.id">
                <button class="row-main clip-row" title="点击复制" @click="copyText(c.content)">
                  <span class="row-text">
                    <span class="clip-text"
                      ><template
                        v-for="(seg, si) in highlight(c.content, clipQuery)"
                        :key="si"
                        ><mark v-if="seg.hit" class="hl">{{ seg.t }}</mark
                        ><template v-else>{{ seg.t }}</template></template
                      ></span
                    >
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
              @input="onSettingInput"
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
                @input="onSettingInput"
              />
              <div class="input-unit">
                <input
                  v-model="speedLimit"
                  class="input"
                  type="number"
                  min="0"
                  placeholder="限速"
                  @input="onSettingInput"
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

        <!-- ========== 安全 ========== -->
        <div v-show="tab === 'security'" class="body">
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
                @click="toggleToken"
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
                @click="toggleAuth"
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
                <button class="op danger" title="删除账号" @click="removeAccount(a.username)">
                  <svg viewBox="0 0 24 24">
                    <path
                      fill="currentColor"
                      d="M6 19a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                    />
                  </svg>
                </button>
              </li>
            </ul>
            <button class="btn primary" @click="openAccModal">添加账号</button>
            <p class="sub">
              账号保存在 <code>~/Pigeon/accounts.json</code> · 密码以加盐哈希存储
            </p>
          </div>
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

    <!-- 添加账号 -->
    <div v-if="showAccModal" class="modal-mask" @click.self="showAccModal = false">
      <div class="modal">
        <h2 class="modal-t">添加账号</h2>
        <div class="field">
          <label>用户名</label>
          <input v-model="newAccUser" class="input" placeholder="用户名" />
        </div>
        <div class="field">
          <label>密码</label>
          <input v-model="newAccPass" class="input" type="password" placeholder="密码" />
        </div>
        <div class="field">
          <label>权限</label>
          <div class="perm-row">
            <label class="check-label"><input v-model="newAccDl" type="checkbox" /> 下载</label>
            <label class="check-label"><input v-model="newAccUf" type="checkbox" /> 上传文件</label>
            <label class="check-label"><input v-model="newAccUt" type="checkbox" /> 上传文本</label>
            <label class="check-label"><input v-model="newAccDel" type="checkbox" /> 删除</label>
          </div>
          <p class="sub">下载 = 浏览文件列表、下载文件、查看记录;删除 = 手机端可删除共享文件</p>
        </div>
        <div class="modal-actions">
          <button class="btn ghost" @click="showAccModal = false">取消</button>
          <button class="btn primary" @click="addAccount">确定</button>
        </div>
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
/* 细滚动条:默认隐形,鼠标悬停滚动区才浮现,不干扰视觉 */
.page ::-webkit-scrollbar,
html::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
.page ::-webkit-scrollbar-track,
html::-webkit-scrollbar-track {
  background: transparent;
}
.page ::-webkit-scrollbar-thumb,
html::-webkit-scrollbar-thumb {
  background: transparent;
  border-radius: 999px;
}
.page .body:hover::-webkit-scrollbar-thumb,
.page .body-scroll:hover::-webkit-scrollbar-thumb,
.page ::-webkit-scrollbar-thumb:hover {
  background: var(--border);
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
.connect-wrap {
  position: relative;
  transition: opacity 0.2s, filter 0.2s;
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
.dz-actions {
  display: flex;
  gap: 8px;
  justify-content: center;
  margin-top: 10px;
}
.dz-btn {
  height: 30px;
  padding: 0 14px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--card);
  color: var(--text-2);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
}
.dz-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-bg);
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

/* 搜索 + 排序工具行 */
.tools-row {
  display: flex;
  gap: 8px;
  margin: 4px 0 10px;
}
.tools-search {
  flex: 1;
  min-width: 0;
  height: 34px;
  border: 1px solid var(--border);
  background: var(--field-bg);
  border-radius: 9px;
  padding: 0 12px;
  font-size: 13px;
  font-family: inherit;
  color: var(--text);
  box-sizing: border-box;
  user-select: text;
  transition: border-color 0.15s;
}
.tools-search::placeholder {
  color: var(--text-4);
}
.tools-search:focus {
  outline: none;
  border-color: var(--accent);
  background: var(--card);
}
.tools-search::-webkit-search-cancel-button {
  -webkit-appearance: none;
}
.sort-btn {
  flex: none;
  height: 34px;
  border: 1px solid var(--border);
  background: var(--card);
  border-radius: 9px;
  padding: 0 12px;
  font-size: 12px;
  color: var(--text-2);
  cursor: pointer;
  font-family: inherit;
  white-space: nowrap;
  transition: background 0.15s, color 0.15s;
}
.sort-btn:hover {
  background: var(--hover);
  color: var(--text);
}

/* 连接排查卡片 */
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

/* 账号管理 */
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
  color: #b7791f;
  background: #fdf6e7;
  border-radius: 8px;
  padding: 8px 10px;
  margin-top: 10px;
}
.page.dark .warn {
  color: #e2b45c;
  background: #2e2617;
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
.acc-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.perm-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}
.check-label {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  color: var(--text-2);
  cursor: pointer;
  user-select: none;
}
.check-label input {
  accent-color: var(--accent);
  width: 14px;
  height: 14px;
  cursor: pointer;
}
.perm-row .sq-btn {
  margin-left: auto;
  height: 34px;
}
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

/* 已冻结(取消分享)折叠面板 */
.frozen-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  border: 0;
  background: none;
  padding: 10px 10px;
  margin-top: 8px;
  border-radius: 8px;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-3);
  cursor: pointer;
  font-family: inherit;
}
.frozen-toggle:hover {
  background: var(--hover);
  color: var(--text-2);
}
.frozen-toggle .chev {
  width: 14px;
  height: 14px;
  transition: transform 0.15s;
}
.frozen-toggle .chev.open {
  transform: rotate(180deg);
}
.frozen-list {
  margin: 0 0 6px;
}
.frozen-list .row-main {
  opacity: 0.7;
}
.frozen-ext {
  color: var(--text-3);
  font-size: 13px;
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
/* 搜索命中高亮 */
.hl {
  background: #ffe27a;
  color: #17203a;
  border-radius: 3px;
  padding: 0 1px;
  margin: 0 -1px;
}
.page.dark .hl {
  background: #8a6d1f;
  color: #fff;
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
