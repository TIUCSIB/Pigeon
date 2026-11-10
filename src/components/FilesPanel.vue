<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { FileEntry, FrozenEntry } from '../types/app'
import { extOf, fmtSize, highlight, timeAgo } from '../utils/format'

const props = defineProps<{ files: FileEntry[]; frozen: FrozenEntry[]; dragging: boolean; uploading: boolean; inboxDir: string | null; resetSelectionKey: number }>()
const emit = defineEmits<{ browse: []; 'browse-folder': []; 'clear-files': []; 'delete-file': [name: string]; 'delete-selected': [names: string[]]; reveal: [name: string]; 'copy-file-url': [name: string]; 'freeze-file': [name: string]; 'unfreeze-file': [name: string]; 'delete-frozen': [name: string]; 'open-folder': [] }>()
const fileQuery = ref('')
const fileSort = ref<'time' | 'name' | 'size'>('time')
const fileSortAsc = ref(false)
const selecting = ref(false)
const selected = ref<Set<string>>(new Set())
const showFrozen = ref(false)
const filteredFiles = computed(() => {
  const q = fileQuery.value.trim().toLowerCase()
  const list = q ? props.files.filter((f) => f.name.toLowerCase().includes(q)) : [...props.files]
  const dir = fileSortAsc.value ? 1 : -1
  return list.sort((a, b) => {
    if (fileSort.value === 'name') return a.name.localeCompare(b.name, 'zh-CN') * dir
    if (fileSort.value === 'size') return (a.size - b.size) * dir
    return (a.modified - b.modified) * dir
  })
})
const allSelected = computed(() => props.files.length > 0 && selected.value.size === props.files.length)
const fileSortLabel = computed(() => ({ time: '按时间', name: '按名称', size: '按大小' })[fileSort.value])
const fileSortDirLabel = computed(() => (fileSortAsc.value ? '↑' : '↓'))
watch(() => props.resetSelectionKey, () => {
  selecting.value = false
  selected.value = new Set()
})
function cycleFileSort() {
  const order: Array<'time' | 'name' | 'size'> = ['time', 'name', 'size']
  fileSort.value = order[(order.indexOf(fileSort.value) + 1) % order.length]
}
function toggleSelectMode() {
  selecting.value = !selecting.value
  selected.value = new Set()
}
function toggleSelected(name: string) {
  const next = new Set(selected.value)
  if (next.has(name)) next.delete(name)
  else next.add(name)
  selected.value = next
}
function toggleSelectAll() {
  selected.value = allSelected.value ? new Set() : new Set(props.files.map((f) => f.name))
}
function requestBatchDelete() {
  emit('delete-selected', Array.from(selected.value))
}
</script>

<template>
<div class="body files-body">
          <div class="body-scroll">
            <div class="drop" :class="{ over: dragging }" @click="$emit('browse')">
              <div class="dz-icon">
                <svg viewBox="0 0 24 24">
                  <path d="M9 16h6v-6h4l-7-7-7 7h4v6zm-4 2h14v2H5z" />
                </svg>
              </div>
              <p class="dz-t">{{ uploading ? '正在上传…' : '拖入文件或文件夹' }}</p>
              <p class="dz-s">拖进窗口、点击选择,或 Ctrl+V 粘贴截图</p>
              <div class="dz-actions">
                <button class="dz-btn" @click.stop="$emit('browse')">选择文件</button>
                <button class="dz-btn" @click.stop="$emit('browse-folder')">选择文件夹</button>
              </div>
              <p class="dz-s">文件夹会打包成 zip 分享</p>
            </div>

            <template v-if="files.length">
              <div class="list-head">
                <span class="sec">已共享</span>
                <span class="sec-n">{{ files.length }}</span>
                <button v-if="!selecting" class="link danger-link" @click="$emit('clear-files')">
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
                    @click="selecting ? toggleSelected(f.name) : $emit('reveal', f.name)"
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
                    <button class="op" title="复制下载链接" @click="$emit('copy-file-url', f.name)">
                      <svg viewBox="0 0 24 24">
                        <path
                          fill="currentColor"
                          d="M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7a5 5 0 0 0 0 10h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4a5 5 0 0 0 0-10z"
                        />
                      </svg>
                    </button>
                    <button class="op" title="取消分享(源文件保留)" @click="$emit('freeze-file', f.name)">
                      <svg viewBox="0 0 24 24">
                        <path
                          fill="currentColor"
                          d="M20.3 17.6c-.3-.6-.9-1-1.5-1.2.4-.6.6-1.3.5-2.1-.2-1.5-1.5-2.7-3-2.9-.7-.1-1.4 0-2 .3C14 10 12.7 8.9 11.1 8.9c-1.9 0-3.5 1.4-3.7 3.3-1.4.2-2.5 1.3-2.7 2.7-.2 1.6.8 3.1 2.3 3.5.3.1.6.1.9.1h10.5c.6 0 1.2-.3 1.7-.7.3-.1.3-.2.2-.2zM12 5.5l2.5 2.5H13v3h-2V8H9.5L12 5.5zM19 14.5l-2.5-2.5H17V9h-2v3h-1.5L16 14.5 19 14.5z"
                          transform="scale(0.9) translate(1.5 1.5)"
                        />
                      </svg>
                    </button>
                    <button class="op danger" title="删除" @click="$emit('delete-file', f.name)">
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
                    <button class="op" title="恢复分享" @click="$emit('unfreeze-file', fr.name)">
                      <svg viewBox="0 0 24 24">
                        <path
                          fill="currentColor"
                          d="M9 16h6v-6h4l-7-7-7 7h4v6zm-4 2h14v2H5z"
                        />
                      </svg>
                    </button>
                    <button class="op danger" title="彻底删除" @click="$emit('delete-frozen', fr.name)">
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
              @click="requestBatchDelete"
            >
              删除选中
            </button>
          </div>
          <div v-else class="dock-bar foot-bar">
            <span class="path" :title="inboxDir">{{ inboxDir }}</span>
            <button class="link" @click="$emit('open-folder')">打开文件夹</button>
          </div>
        </div>
</template>

<style scoped>
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
.row-main.dim {
  opacity: 0.75;
}
li:has(.check.on) .row-main {
  background: var(--accent-bg);
}
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
  transition: background 0.12s, border-color 0.12s;
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
.select-all-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 10px 6px 0;
}
.sa-t {
  font-size: 12px;
  color: var(--text-3);
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
.batch-n {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.dock-bar .btn.primary.danger:disabled {
  background: var(--accent-bg);
  color: var(--text-4);
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
</style>
