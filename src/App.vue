<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import { getCurrentWindow } from '@tauri-apps/api/window'
import QRCode from 'qrcode'
import AppTitlebar from './components/AppTitlebar.vue'
import AppTabs from './components/AppTabs.vue'
import ConnectPanel from './components/ConnectPanel.vue'
import FilesPanel from './components/FilesPanel.vue'
import ClipsPanel from './components/ClipsPanel.vue'
import DevicesPanel from './components/DevicesPanel.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import SecurityPanel from './components/SecurityPanel.vue'
import ToastStack from './components/ToastStack.vue'
import DragOverlay from './components/DragOverlay.vue'
import AddAccountDialog from './components/AddAccountDialog.vue'
import ConfirmDialog from './components/ConfirmDialog.vue'

import type { AccountInput, AccountView, Clip, ConfirmKind, DeviceEntry, FileEntry, FrozenEntry, ServerInfo, Settings, Tab, Toast, TransferRecord } from './types/app'
import { fmtSize } from './utils/format'

const tab = ref<Tab>('connect')
const info = ref<ServerInfo | null>(null)
const qr = ref('')
const files = ref<FileEntry[]>([])
const clips = ref<Clip[]>([])

const dragging = ref(false)
const uploading = ref(false)
const textInput = ref('')
const sending = ref(false)
const toasts = ref<Toast[]>([])
const deleteTarget = ref<string | null>(null)
// 确认弹窗类型:file=删单个文件 clips-clear=清空记录 files-clear=清空文件 files-batch=批量删除选中
const confirmKind = ref<ConfirmKind | null>(null)
const showAccModal = ref(false)

// 桌面 UI 自己调 HTTP 上传接口(与手机端同通道)
const uploadUrl = ref('')
const batchDeleteNames = ref<string[]>([])
const selectionResetKey = ref(0)

async function deleteSelected(names: string[]) {
  const targets = Array.from(names)
  confirmKind.value = null
  let ok = 0
  for (const name of targets) {
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

// 设置
const settingsPort = ref('')
const settingsDir = ref('')
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
// 冻结文件列表(多选模式或折叠面板展示)
const frozen = ref<FrozenEntry[]>([])
// 连接过的设备 / 传输历史(桌面"连接"页展示)
const devices = ref<DeviceEntry[]>([])
const history = ref<TransferRecord[]>([])
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

async function refreshDevices() {
  try {
    devices.value = await invoke<DeviceEntry[]>('list_devices')
    history.value = await invoke<TransferRecord[]>('list_history')
  } catch {
    // ignore
  }
}

async function clearHistory() {
  try {
    await invoke('clear_history')
    toast('传输历史已清空')
    await refreshDevices()
  } catch (e) {
    toast(`${e}`)
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
async function openFirewall() {
  try {
    await invoke('open_firewall_settings')
  } catch (e) {
    toast(`${e}`)
  }
}

let toastSeq = 0
const unlisteners: UnlistenFn[] = []

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
    await deleteSelected(batchDeleteNames.value)
    selectionResetKey.value++
  }
  if (confirmKind.value === 'history-clear') {
    confirmKind.value = null
    await clearHistory()
  }
}

function requestFileDelete(name: string) {
  deleteTarget.value = name
  confirmKind.value = 'file'
}

function requestBatchDelete(names: string[]) {
  batchDeleteNames.value = names
  confirmKind.value = 'files-batch'
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
  // 镜像到 localStorage:下次启动 index.html 的启动画面在 Vue 挂载前就能读到,跟随主题
  try {
    localStorage.setItem('pigeon:dark', dark ? '1' : '0')
  } catch {}
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

async function addAccount(account: AccountInput) {
  try {
    await invoke('add_account', {
      username: account.username,
      password: account.password,
      allowDownload: account.allowDownload,
      allowUploadFile: account.allowUploadFile,
      allowUploadText: account.allowUploadText,
      allowDelete: account.allowDelete,
    })
    showAccModal.value = false
    await refreshAccounts()
    toast('账号已添加')
  } catch (e) {
    toast(`${e}`)
  }
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
// 连接页可见时的设备/历史轮询计时器
let devTimer: ReturnType<typeof setInterval> | undefined

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
    refreshDevices(),
  ])

  // 设备页可见时轻量轮询:在线状态/传输历史没有推送通道,
  // 本地 invoke 开销可忽略;切走立即停
  watch(
    tab,
    (t) => {
      clearInterval(devTimer)
      devTimer = undefined
      if (t === 'devices') {
        void refreshDevices()
        devTimer = setInterval(refreshDevices, 5000)
      }
    },
    { immediate: true },
  )

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
      await refreshDevices()
      // 手机发来的才弹提示;桌面自己的操作(拖拽/选择/截图)已有各自的 toast
      if (event.payload.source === 'phone' && !uploading.value) {
        toast(`收到 ${event.payload.name}(${fmtSize(event.payload.size)})`)
      }
    }),
  )

  unlisteners.push(
    await listen<Clip>('clip-added', async (event) => {
      await refreshClips()
      await refreshDevices()
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
  clearInterval(devTimer)
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
      <AppTitlebar
        :sharing="sharing"
        @toggle-sharing="toggleSharing"
        @minimize="winMinimize"
        @close="winClose"
      />
      <div class="card">
        <AppTabs v-model="tab" :file-count="files.length" />

        <ConnectPanel
          v-show="tab === 'connect'"
          v-model:text-input="textInput"
          :sharing="sharing"
          :qr="qr"
          :server-info="info"
          :sending="sending"
          @copy-url="copyUrl"
          @send-text="sendText"
          @send-clipboard-text="sendClipboardText"
          @toggle-sharing="toggleSharing"
          @open-firewall="openFirewall"
        />

        <FilesPanel
          v-show="tab === 'files'"
          :files="files"
          :frozen="frozen"
          :dragging="dragging"
          :uploading="uploading"
          :inbox-dir="info?.inbox_dir ?? null"
          :reset-selection-key="selectionResetKey"
          @browse="browse"
          @browse-folder="browseFolder"
          @clear-files="confirmKind = 'files-clear'"
          @delete-file="requestFileDelete"
          @delete-selected="requestBatchDelete"
          @reveal="reveal"
          @copy-file-url="copyFileUrl"
          @freeze-file="freezeFile"
          @unfreeze-file="unfreezeFile"
          @delete-frozen="deleteFrozen"
          @open-folder="openFolder"
        />

        <ClipsPanel
          v-show="tab === 'clips'"
          :clips="clips"
          @clear-clips="confirmKind = 'clips-clear'"
          @copy-text="copyText"
          @delete-clip="deleteClip"
        />

        <DevicesPanel
          v-show="tab === 'devices'"
          :devices="devices"
          :history="history"
          :server-info="info"
          @clear-history="confirmKind = 'history-clear'"
        />

        <SettingsPanel
          v-show="tab === 'settings'"
          v-model:dark-mode="darkMode"
          v-model:port="settingsPort"
          v-model:dir="settingsDir"
          v-model:max-concurrent="maxConcurrent"
          v-model:speed-limit="speedLimit"
          @toggle-dark="toggleDark"
          @choose-dir="chooseDir"
          @setting-input="onSettingInput"
        />

        <SecurityPanel
          v-show="tab === 'security'"
          :token-enabled="tokenEnabled"
          :auth-enabled="authEnabled"
          :accounts="accounts"
          @toggle-token="toggleToken"
          @toggle-auth="toggleAuth"
          @remove-account="removeAccount"
          @open-account-dialog="showAccModal = true"
        />
      </div>
    </div>

    <ToastStack :toasts="toasts" />
    <DragOverlay v-if="dragging" />
    <AddAccountDialog
      v-model:open="showAccModal"
      @submit="addAccount"
      @invalid="toast('用户名和密码不能为空')"
    />
    <ConfirmDialog
      :kind="confirmKind"
      :delete-target="deleteTarget"
      :clips-count="clips.length"
      :selected-count="batchDeleteNames.length"
      :files-count="files.length"
      @cancel="confirmKind = null"
      @confirm="doDelete"
    />
  </div>
</template>
