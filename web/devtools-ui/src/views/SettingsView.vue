<script setup lang="ts">
import { NAlert, NButton, NCard, NList, NListItem, NSwitch, NTag } from 'naive-ui';

import type { Settings } from '../ipc/types';

defineOptions({ name: 'SettingsView' });

const props = defineProps<{
  readonly settings: Settings;
  readonly version: string;
  readonly canGoBack: boolean;
  readonly error: string | null;
}>();

const emit = defineEmits<{
  back: [];
  update: [settings: Settings];
}>();

function updateShowTray(showTray: boolean): void {
  emit('update', { ...props.settings, showTray });
}

function updateAutostart(autostart: boolean): void {
  emit('update', { ...props.settings, autostart });
}
</script>

<template>
  <main class="settings-view">
    <header class="settings-view__header">
      <div>
        <h1>DevTools 设置</h1>
        <p>Worker 与桌面集成</p>
      </div>
      <NButton v-if="canGoBack" secondary @click="emit('back')">返回 JSON</NButton>
    </header>

    <NAlert v-if="error !== null" title="设置保存失败" type="error">
      {{ error }}
    </NAlert>

    <NCard title="常规" :bordered="false">
      <NList>
        <NListItem>
          <div class="settings-view__item">
            <div>
              <strong>显示任务栏图标</strong>
              <p>在 KDE 系统托盘显示 DevTools，并提供设置、重启和退出菜单。</p>
            </div>
            <NSwitch :value="settings.showTray" @update:value="updateShowTray" />
          </div>
        </NListItem>
        <NListItem>
          <div class="settings-view__item">
            <div>
              <strong>开机自启动</strong>
              <p>登录当前 KDE 用户会话后启动 DevTools Worker。</p>
            </div>
            <NSwitch :value="settings.autostart" @update:value="updateAutostart" />
          </div>
        </NListItem>
      </NList>
    </NCard>

    <NAlert title="运行环境" type="info" :show-icon="false">
      <div class="settings-view__runtime">
        <span>版本</span>
        <NTag :bordered="false">{{ version }}</NTag>
        <span>平台</span>
        <NTag :bordered="false">Debian 13 · KDE · Wayland / X11</NTag>
      </div>
    </NAlert>
  </main>
</template>

<style scoped lang="scss">
.settings-view {
  display: grid;
  gap: 1rem;
  margin: 0 auto;
  max-width: 52rem;
  padding: 1.5rem;

  &__header,
  &__item,
  &__runtime {
    align-items: center;
    display: flex;
    gap: 1rem;
    justify-content: space-between;
  }

  &__header {
    h1,
    p {
      margin: 0;
    }

    p {
      color: var(--muted-color);
    }
  }

  &__item {
    width: 100%;

    p {
      color: var(--muted-color);
      margin: 0.35rem 0 0;
    }
  }

  &__runtime {
    justify-content: flex-start;
  }
}
</style>
