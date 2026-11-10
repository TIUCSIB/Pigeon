<script setup lang="ts">
import type { ConfirmKind } from '../types/app'

defineProps<{ kind: ConfirmKind | null; deleteTarget: string | null; clipsCount: number; selectedCount: number; filesCount: number }>()
defineEmits<{ cancel: []; confirm: [] }>()
</script>

<template>
<div v-if="kind" class="modal-mask" @click.self="$emit('cancel')">
      <div class="modal">
        <h2 class="modal-t">
          {{
            kind === 'file'
              ? '删除文件'
              : kind === 'clips-clear'
                ? '清空文字记录'
                : kind === 'files-batch'
                  ? '删除选中文件'
                  : '清空共享文件'
          }}
        </h2>
        <p v-if="kind === 'file'" class="modal-desc">
          确定删除 <b>{{ deleteTarget }}</b> ?手机端将无法再下载该文件。
        </p>
        <p v-else-if="kind === 'clips-clear'" class="modal-desc">
          确定清空全部 {{ clipsCount }} 条文字记录?此操作不可恢复。
        </p>
        <p v-else-if="kind === 'files-batch'" class="modal-desc">
          确定删除选中的 {{ selectedCount }} 个文件?此操作不可恢复。
        </p>
        <p v-else class="modal-desc">
          确定清空共享目录中的 {{ filesCount }} 个文件?手机端将无法再下载,此操作不可恢复。
        </p>
        <div class="modal-actions">
          <button class="btn ghost" @click="$emit('cancel')">取消</button>
          <button class="btn primary danger" @click="$emit('confirm')">删除</button>
        </div>
      </div>
    </div>
</template>
