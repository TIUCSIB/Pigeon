<template>
<div class="drag-overlay">
      <div class="drag-box">
        <p class="drag-t">松开,发送到手机</p>
        <p class="drag-s">文件夹将自动打包成 zip</p>
      </div>
    </div>
</template>

<style scoped>
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
</style>
