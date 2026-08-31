<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { darkTheme, NConfigProvider, NMessageProvider, useOsTheme, zhCN } from 'naive-ui';

import { postRequest } from './ipc/bridge';
import type { OpenConvertDetail, OpenJsonDetail, Settings, SettingsDetail } from './ipc/types';
import type { FormatId } from './tools/converter/types';
import BarcodeStudioView from './views/BarcodeStudioView.vue';
import DataConvertView from './views/DataConvertView.vue';
import JsonWorkbench from './views/JsonWorkbench.vue';
import OcrView from './views/OcrView.vue';
import SettingsView from './views/SettingsView.vue';

defineOptions({ name: 'App' });

type View = 'barcode' | 'convert' | 'idle' | 'json' | 'ocr' | 'settings';

const initialState = window.__DEVTOOLS_INITIAL_STATE__ ?? {
  version: 'development',
  settings: { showTray: true, autostart: false },
  converterCapabilities: { nativeFormats: [], phpVersion: null },
  mediaCapabilities: {
    ocr: { available: false, version: null, languages: [] },
    barcode: { available: false, version: null },
  },
};
const osTheme = useOsTheme();
const theme = computed(() => (osTheme.value === 'dark' ? darkTheme : null));
const view = ref<View>('idle');
const previousView = ref<View>('idle');
const jsonPayload = ref<string>('');
const convertPayload = ref<string>('');
const convertSourceHint = ref<FormatId | null>(null);
const convertActivation = ref<number>(0);
const convertCanGoBack = ref<boolean>(false);
const settings = ref<Settings>(initialState.settings);
const settingsError = ref<string | null>(null);

function handleOpenJson(event: Event): void {
  if (!(event instanceof CustomEvent) || !isOpenJsonDetail(event.detail)) {
    return;
  }
  jsonPayload.value = event.detail.payload;
  view.value = 'json';
}

function handleOpenSettings(): void {
  previousView.value = view.value;
  view.value = 'settings';
  postRequest({ type: 'settingsGet' });
}

function handleOpenConvert(event: Event): void {
  if (!(event instanceof CustomEvent) || !isOpenConvertDetail(event.detail)) {
    return;
  }
  openConvert(event.detail.payload, null, false);
}

function handleOpenOcr(): void {
  view.value = 'ocr';
}

function handleOpenBarcode(): void {
  view.value = 'barcode';
}

function openConvert(payload: string, sourceHint: FormatId | null, canGoBack: boolean): void {
  convertPayload.value = payload;
  convertSourceHint.value = sourceHint;
  convertCanGoBack.value = canGoBack;
  convertActivation.value += 1;
  view.value = 'convert';
}

function openConvertFromJson(payload: string): void {
  openConvert(payload, 'json', true);
}

function backToJson(): void {
  view.value = 'json';
}

function handleSettings(event: Event): void {
  if (!(event instanceof CustomEvent) || !isSettingsDetail(event.detail)) {
    return;
  }
  settings.value = event.detail.settings;
  settingsError.value = event.detail.error;
}

function updateSettings(nextSettings: Settings): void {
  settings.value = nextSettings;
  postRequest({ type: 'settingsUpdate', settings: nextSettings });
}

function goBack(): void {
  view.value = previousView.value === 'settings' ? 'idle' : previousView.value;
}

function isOpenJsonDetail(detail: unknown): detail is OpenJsonDetail {
  return isRecord(detail) && typeof detail['payload'] === 'string';
}

function isOpenConvertDetail(detail: unknown): detail is OpenConvertDetail {
  return isRecord(detail) && typeof detail['payload'] === 'string';
}

function isSettingsDetail(detail: unknown): detail is SettingsDetail {
  if (!isRecord(detail) || !isRecord(detail['settings'])) {
    return false;
  }
  const candidate = detail['settings'];
  const error = detail['error'];
  return (
    typeof candidate['showTray'] === 'boolean' &&
    typeof candidate['autostart'] === 'boolean' &&
    (error === null || typeof error === 'string')
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

onMounted(() => {
  window.addEventListener('devtools:open-json', handleOpenJson);
  window.addEventListener('devtools:open-convert', handleOpenConvert);
  window.addEventListener('devtools:open-ocr', handleOpenOcr);
  window.addEventListener('devtools:open-barcode', handleOpenBarcode);
  window.addEventListener('devtools:open-settings', handleOpenSettings);
  window.addEventListener('devtools:settings', handleSettings);
});

onBeforeUnmount(() => {
  window.removeEventListener('devtools:open-json', handleOpenJson);
  window.removeEventListener('devtools:open-convert', handleOpenConvert);
  window.removeEventListener('devtools:open-ocr', handleOpenOcr);
  window.removeEventListener('devtools:open-barcode', handleOpenBarcode);
  window.removeEventListener('devtools:open-settings', handleOpenSettings);
  window.removeEventListener('devtools:settings', handleSettings);
});
</script>

<template>
  <NConfigProvider :locale="zhCN" :theme="theme">
    <NMessageProvider>
      <JsonWorkbench
        v-if="jsonPayload !== ''"
        v-show="view === 'json'"
        :payload="jsonPayload"
        @convert="openConvertFromJson"
      />
      <DataConvertView
        v-if="view === 'convert'"
        :activation="convertActivation"
        :can-go-back="convertCanGoBack"
        :capabilities="initialState.converterCapabilities"
        :payload="convertPayload"
        :source-hint="convertSourceHint"
        @back="backToJson"
      />
      <OcrView v-if="view === 'ocr'" :capability="initialState.mediaCapabilities.ocr" />
      <BarcodeStudioView
        v-if="view === 'barcode'"
        :capability="initialState.mediaCapabilities.barcode"
      />
      <SettingsView
        v-if="view === 'settings'"
        :can-go-back="previousView !== 'idle' && previousView !== 'settings'"
        :error="settingsError"
        :settings="settings"
        :version="initialState.version"
        @back="goBack"
        @update="updateSettings"
      />
      <main v-if="view === 'idle'" class="idle-view">
        <h1>DevTools Worker</h1>
        <p>请从 KRunner 输入 JSON、convert、ocr 或 barcode，或从任务栏图标打开设置。</p>
      </main>
    </NMessageProvider>
  </NConfigProvider>
</template>
