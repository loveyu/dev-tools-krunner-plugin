<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import {
  darkTheme,
  enUS,
  NConfigProvider,
  NMessageProvider,
  useOsTheme,
  zhCN,
  zhTW,
} from 'naive-ui';

import { provideI18n, resolveLocale, translate } from './i18n/runtime';
import { postRequest } from './ipc/bridge';
import type { OpenConvertDetail, OpenJsonDetail, Settings, SettingsDetail } from './ipc/types';
import type { FormatId } from './tools/converter/types';
import type { LauncherAction, LauncherToolId } from './tools/launcher/model';
import BarcodeStudioView from './views/BarcodeStudioView.vue';
import DataConvertView from './views/DataConvertView.vue';
import ImageCompressionView from './views/ImageCompressionView.vue';
import ImageEditorView from './views/ImageEditorView.vue';
import JsonWorkbench from './views/JsonWorkbench.vue';
import LauncherView from './views/LauncherView.vue';
import OcrView from './views/OcrView.vue';
import SettingsView from './views/SettingsView.vue';
import WatermarkView from './views/WatermarkView.vue';

defineOptions({ name: 'App' });

type View =
  | 'barcode'
  | 'convert'
  | 'idle'
  | 'image-compress'
  | 'image-editor'
  | 'json'
  | 'launcher'
  | 'ocr'
  | 'settings'
  | 'watermark';

const initialState = window.__DEVTOOLS_INITIAL_STATE__ ?? {
  version: 'development',
  settings: {
    showTray: true,
    autostart: false,
    globalShortcutEnabled: false,
    globalShortcut: 'Ctrl+Alt+Space',
    quickInputEnabled: false,
    quickInputShortcut: 'Ctrl+Alt+KeyI',
    quickInputWidth: 560,
    quickInputHeight: 56,
    theme: 'system',
    language: 'system',
  },
  converterCapabilities: { nativeFormats: [], phpVersion: null },
  mediaCapabilities: {
    ocr: { available: false, version: null, languages: [] },
    barcode: { available: false, version: null },
  },
};
const settings = ref<Settings>(initialState.settings);
const systemLanguages = ref<readonly string[]>(Array.from(navigator.languages));
const locale = computed(() => resolveLocale(settings.value.language, systemLanguages.value));
provideI18n(locale);
const osTheme = useOsTheme();
const theme = computed(() => {
  const mode = settings.value.theme;
  return mode === 'dark' || (mode === 'system' && osTheme.value === 'dark') ? darkTheme : null;
});
const naiveLocale = computed(() => {
  if (locale.value === 'zh-CN') return zhCN;
  if (locale.value === 'zh-TW') return zhTW;
  return enUS;
});
const view = ref<View>('idle');
const previousView = ref<View>('idle');
const jsonPayload = ref<string>('');
const convertPayload = ref<string>('');
const convertSourceHint = ref<FormatId | null>(null);
const convertActivation = ref<number>(0);
const convertCanGoBack = ref<boolean>(false);
const settingsError = ref<string | null>(null);
const launcherActivation = ref(0);

watch(
  () => settings.value.theme,
  (mode) => {
    document.documentElement.dataset['theme'] = mode;
  },
  { immediate: true },
);

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

function handleOpenLauncher(): void {
  launcherActivation.value += 1;
  view.value = 'launcher';
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

function handleOpenImageCompress(): void {
  view.value = 'image-compress';
}

function handleOpenImageEditor(): void {
  view.value = 'image-editor';
}

function handleOpenWatermark(): void {
  view.value = 'watermark';
}

function handleLanguageChange(): void {
  systemLanguages.value = Array.from(navigator.languages);
}

function t(key: string): string {
  return translate(locale.value, key);
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

function activateLauncher(action: LauncherAction): void {
  if (action.type === 'open-settings') {
    handleOpenSettings();
    return;
  }
  openLauncherTool(action.tool, action.payload);
}

function openLauncherTool(tool: LauncherToolId, payload: string): void {
  switch (tool) {
    case 'json':
      jsonPayload.value = payload === '' ? '{}' : payload;
      view.value = 'json';
      break;
    case 'convert':
      openConvert(payload, null, false);
      break;
    case 'ocr':
      handleOpenOcr();
      break;
    case 'barcode':
      handleOpenBarcode();
      break;
    case 'image-compress':
      handleOpenImageCompress();
      break;
    case 'image-editor':
      handleOpenImageEditor();
      break;
    case 'watermark':
      handleOpenWatermark();
      break;
  }
}

function closeWindow(): void {
  postRequest({ type: 'windowHide' });
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
    typeof candidate['globalShortcutEnabled'] === 'boolean' &&
    typeof candidate['globalShortcut'] === 'string' &&
    typeof candidate['quickInputEnabled'] === 'boolean' &&
    typeof candidate['quickInputShortcut'] === 'string' &&
    typeof candidate['quickInputWidth'] === 'number' &&
    typeof candidate['quickInputHeight'] === 'number' &&
    isThemeMode(candidate['theme']) &&
    isLanguageMode(candidate['language']) &&
    (error === null || typeof error === 'string')
  );
}

function isThemeMode(value: unknown): value is Settings['theme'] {
  return value === 'system' || value === 'light' || value === 'dark';
}

function isLanguageMode(value: unknown): value is Settings['language'] {
  return value === 'system' || value === 'zh-CN' || value === 'zh-TW' || value === 'en-US';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

onMounted(() => {
  window.addEventListener('devtools:open-json', handleOpenJson);
  window.addEventListener('devtools:open-convert', handleOpenConvert);
  window.addEventListener('devtools:open-ocr', handleOpenOcr);
  window.addEventListener('devtools:open-barcode', handleOpenBarcode);
  window.addEventListener('devtools:open-image-compress', handleOpenImageCompress);
  window.addEventListener('devtools:open-image-editor', handleOpenImageEditor);
  window.addEventListener('devtools:open-watermark', handleOpenWatermark);
  window.addEventListener('devtools:open-settings', handleOpenSettings);
  window.addEventListener('devtools:open-launcher', handleOpenLauncher);
  window.addEventListener('devtools:settings', handleSettings);
  window.addEventListener('languagechange', handleLanguageChange);
});

onBeforeUnmount(() => {
  window.removeEventListener('devtools:open-json', handleOpenJson);
  window.removeEventListener('devtools:open-convert', handleOpenConvert);
  window.removeEventListener('devtools:open-ocr', handleOpenOcr);
  window.removeEventListener('devtools:open-barcode', handleOpenBarcode);
  window.removeEventListener('devtools:open-image-compress', handleOpenImageCompress);
  window.removeEventListener('devtools:open-image-editor', handleOpenImageEditor);
  window.removeEventListener('devtools:open-watermark', handleOpenWatermark);
  window.removeEventListener('devtools:open-settings', handleOpenSettings);
  window.removeEventListener('devtools:open-launcher', handleOpenLauncher);
  window.removeEventListener('devtools:settings', handleSettings);
  window.removeEventListener('languagechange', handleLanguageChange);
});
</script>

<template>
  <NConfigProvider :locale="naiveLocale" :theme="theme">
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
      <ImageCompressionView v-if="view === 'image-compress'" />
      <ImageEditorView v-if="view === 'image-editor'" />
      <WatermarkView v-if="view === 'watermark'" />
      <LauncherView
        v-if="view === 'launcher'"
        :activation="launcherActivation"
        @activate="activateLauncher"
        @close="closeWindow"
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
        <p>
          {{ t('ui.openToolsFromLauncherOrKrunner') }}
        </p>
      </main>
    </NMessageProvider>
  </NConfigProvider>
</template>
