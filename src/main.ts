import { createApp } from 'vue'
import App from './App.vue'

const app = createApp(App)
app.mount('#app')

// Vue 已接管界面,淡出并移除 index.html 里的启动画面
const splash = document.getElementById('splash')
if (splash) {
  splash.style.opacity = '0'
  splash.addEventListener('transitionend', () => splash.remove(), { once: true })
  // transitionend 可能因系统设置(减少动画)不触发,兜底移除
  setTimeout(() => splash.remove(), 500)
}
