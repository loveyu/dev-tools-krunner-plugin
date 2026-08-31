import { createApp } from 'vue';

import App from './App.vue';
import { postRequest } from './ipc/bridge';
import './styles/main.scss';

const pendingEvents = window.__DEVTOOLS_PENDING_EVENTS__ ?? [];
window.__DEVTOOLS_DISPATCH__ = (name: string, detail: unknown): void => {
  window.dispatchEvent(new CustomEvent(name, { detail }));
};

// 类型感知 ESLint 不解析 Vue SFC；组件类型由独立的 vue-tsc 门禁负责。
// eslint-disable-next-line @typescript-eslint/no-unsafe-argument
createApp(App).mount('#app');

for (const event of pendingEvents) {
  window.__DEVTOOLS_DISPATCH__(event.name, event.detail);
}
window.__DEVTOOLS_PENDING_EVENTS__ = [];
postRequest({ type: 'frontendReady' });
