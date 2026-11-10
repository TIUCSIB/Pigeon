<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Clip } from '../types/app'
import { fmtClipTime, highlight } from '../utils/format'

const props = defineProps<{ clips: Clip[] }>()
defineEmits<{ 'clear-clips': []; 'copy-text': [text: string]; 'delete-clip': [clip: Clip] }>()
const clipQuery = ref('')
const clipSort = ref<'time' | 'name'>('time')
const filteredClips = computed(() => {
  const q = clipQuery.value.trim().toLowerCase()
  const list = q ? props.clips.filter((c) => c.content.toLowerCase().includes(q)) : [...props.clips]
  if (clipSort.value === 'name') return list.sort((a, b) => a.content.localeCompare(b.content, 'zh-CN'))
  return list.sort((a, b) => b.created - a.created)
})
const clipSortLabel = computed(() => (clipSort.value === 'time' ? '按时间' : '按名称'))
function cycleClipSort() {
  clipSort.value = clipSort.value === 'time' ? 'name' : 'time'
}
</script>

<template>
<div class="body">
          <template v-if="clips.length">
            <div class="list-head">
              <span class="sec">文字记录</span>
              <span class="sec-n">{{ clips.length }}</span>
              <button class="link danger-link" @click="$emit('clear-clips')">清空</button>
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
                <button class="row-main clip-row" title="点击复制" @click="$emit('copy-text', c.content)">
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
                  <button class="op danger" title="删除" @click="$emit('delete-clip', c)">
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
</template>

<style scoped>
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
</style>
