<script setup lang="ts">
import { ref, watch } from 'vue'
import type { AccountInput } from '../types/app'

const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ submit: [account: AccountInput]; invalid: [] }>()
const newAccUser = ref('')
const newAccPass = ref('')
const newAccDl = ref(true)
const newAccUf = ref(true)
const newAccUt = ref(true)
const newAccDel = ref(false)
function resetForm() {
  newAccUser.value = ''
  newAccPass.value = ''
  newAccDl.value = true
  newAccUf.value = true
  newAccUt.value = true
  newAccDel.value = false
}
watch(open, (value) => { if (value) resetForm() })
function submit() {
  if (!newAccUser.value.trim() || !newAccPass.value) {
    emit('invalid')
    return
  }
  emit('submit', {
    username: newAccUser.value.trim(),
    password: newAccPass.value,
    allowDownload: newAccDl.value,
    allowUploadFile: newAccUf.value,
    allowUploadText: newAccUt.value,
    allowDelete: newAccDel.value,
  })
}
</script>

<template>
<div v-if="open" class="modal-mask" @click.self="open = false">
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
            <label class="check-label">
              <input v-model="newAccDl" type="checkbox" /><span>下载</span>
            </label>
            <label class="check-label">
              <input v-model="newAccUf" type="checkbox" /><span>上传文件</span>
            </label>
            <label class="check-label">
              <input v-model="newAccUt" type="checkbox" /><span>上传文本</span>
            </label>
            <label class="check-label">
              <input v-model="newAccDel" type="checkbox" /><span>删除</span>
            </label>
          </div>
          <p class="sub">下载 = 浏览文件列表、下载文件、查看记录;删除 = 手机端可删除共享文件</p>
        </div>
        <div class="modal-actions">
          <button class="btn ghost" @click="open = false">取消</button>
          <button class="btn primary" @click="submit">确定</button>
        </div>
      </div>
    </div>
</template>

<style scoped>
.check-label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  line-height: 15px;
  color: var(--text-2);
  cursor: pointer;
  user-select: none;
}

.check-label span {
  display: inline-flex;
  align-items: center;
  height: 15px;
  line-height: 15px;
}

.check-label input {
  appearance: none;
  -webkit-appearance: none;
  width: 15px;
  height: 15px;
  margin: 0;
  flex: none;
  border: 1.5px solid var(--border);
  border-radius: 4.5px;
  background: var(--card);
  cursor: pointer;
  position: relative;
  box-sizing: border-box;
  transition:
    background 0.12s,
    border-color 0.12s;
}

.check-label input:hover {
  border-color: var(--accent);
}

.check-label input:checked {
  background: var(--accent);
  border-color: var(--accent);
}

.check-label input:checked::after {
  content: '';
  position: absolute;
  left: 4px;
  top: 1px;
  width: 4px;
  height: 7.5px;
  border: solid #fff;
  border-width: 0 1.5px 1.5px 0;
  transform: rotate(45deg);
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
</style>
