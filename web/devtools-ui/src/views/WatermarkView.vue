<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, provide, reactive, ref, watch } from 'vue';
import { NAlert, NCard, NEmpty, NTag, useMessage } from 'naive-ui';

import WatermarkSettingsPanel from '../components/WatermarkSettingsPanel.vue';
import { useI18n } from '../i18n/runtime';
import { validateImageMetadata, SUPPORTED_IMAGE_TYPES } from '../tools/media/image';
import {
  DEFAULT_TIME_TEMPLATE,
  WATERMARKER_PROJECT_URL,
  WATERMARK_SETTINGS_KEY,
  createDefaultWatermarkSettings,
  watermarkedFilename,
} from '../tools/watermark/model';
import { composeWatermark, loadImageElement, renderWatermark } from '../tools/watermark/renderer';

defineOptions({ name: 'WatermarkView' });

const message = useMessage();
const { t } = useI18n();

const sourceInput = ref<HTMLInputElement | null>(null);
const watermarkInput = ref<HTMLInputElement | null>(null);
const sourceFile = ref<File | null>(null);
const sourceImage = ref<HTMLImageElement | null>(null);
const watermarkFile = ref<File | null>(null);
const watermarkImage = ref<HTMLImageElement | null>(null);
const previewCanvasHost = ref<HTMLElement | null>(null);
const hasPreview = ref(false);
const saving = ref(false);
const error = ref<string | null>(null);

/** 默认文案对齐原项目：两行说明 + 一行时间模板。 */
function defaultWatermarkText(): string {
  return `${t('watermark.defaults.line1')}\n${t('watermark.defaults.line2')}\n${DEFAULT_TIME_TEMPLATE}`;
}

const settings = reactive(createDefaultWatermarkSettings(defaultWatermarkText()));
// 设置面板通过 inject 直接修改该对象，驱动实时预览。
provide(WATERMARK_SETTINGS_KEY, settings);

const hasTimeTemplate = computed(() => /\{[YMDhms]\}/.test(settings.text));
const canSave = computed(() => {
  if (sourceImage.value === null) return false;
  return settings.mode === 'text' ? settings.text.trim() !== '' : watermarkImage.value !== null;
});

let previewTimer: number | null = null;
let clockTimer: number | null = null;

onMounted(() => {
  window.addEventListener('paste', handlePaste);
  schedulePreview();
});

onBeforeUnmount(() => {
  window.removeEventListener('paste', handlePaste);
  if (previewTimer !== null) window.clearTimeout(previewTimer);
  if (clockTimer !== null) window.clearInterval(clockTimer);
});

// 水印含时间占位符时按秒刷新预览，模拟真实渲染时刻。
watch(
  hasTimeTemplate,
  (active) => {
    startClock(active);
  },
  { immediate: true },
);

watch(
  settings,
  () => {
    schedulePreview();
  },
  { deep: true },
);

function startClock(active: boolean): void {
  if (clockTimer !== null) {
    window.clearInterval(clockTimer);
    clockTimer = null;
  }
  if (active) {
    clockTimer = window.setInterval(() => {
      schedulePreview();
    }, 1000);
  }
}

function schedulePreview(): void {
  if (previewTimer !== null) window.clearTimeout(previewTimer);
  previewTimer = window.setTimeout(() => {
    renderPreview();
  }, 120);
}

/** 预览直接输出 canvas 元素，跳过耗时的图片编码；编码只发生在保存/复制时。 */
function renderPreview(): void {
  const source = sourceImage.value;
  const host = previewCanvasHost.value;
  if (source === null || host === null) return;
  try {
    const canvas = composeWatermark({
      source,
      watermarkImage: watermarkImage.value,
      settings: { ...settings, textStyle: { ...settings.textStyle } },
      now: new Date(),
      preview: true,
    });
    canvas.className = 'watermark-view__preview-canvas';
    host.replaceChildren(canvas);
    hasPreview.value = true;
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
  }
}

function openSourcePicker(): void {
  sourceInput.value?.click();
}

function openWatermarkPicker(): void {
  watermarkInput.value?.click();
}

function handleSourceInput(event: Event): void {
  const input = event.target;
  if (!(input instanceof HTMLInputElement) || input.files === null) return;
  const file = firstImageFile(input.files);
  if (file !== null) void selectSource(file);
  input.value = '';
}

function handleWatermarkInput(event: Event): void {
  const input = event.target;
  if (!(input instanceof HTMLInputElement) || input.files === null) return;
  const file = firstImageFile(input.files);
  if (file !== null) void selectWatermark(file);
  input.value = '';
}

function handleDrop(event: DragEvent): void {
  const file = event.dataTransfer === null ? null : firstImageFile(event.dataTransfer.files);
  if (file !== null) void selectSource(file);
}

function handlePaste(event: ClipboardEvent): void {
  if (event.clipboardData === null) return;
  const file = firstImageFile(event.clipboardData.files);
  if (file === null) return;
  event.preventDefault();
  void selectSource(file);
}

async function selectSource(file: File): Promise<void> {
  try {
    const validationError = validateImageMetadata(file.type, file.size);
    if (validationError !== null) {
      error.value = validationError;
      return;
    }
    const image = await loadImageElement(file);
    sourceFile.value = file;
    sourceImage.value = image;
    error.value = null;
    schedulePreview();
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
  }
}

async function selectWatermark(file: File): Promise<void> {
  try {
    const validationError = validateImageMetadata(file.type, file.size);
    if (validationError !== null) {
      error.value = validationError;
      return;
    }
    const image = await loadImageElement(file);
    watermarkFile.value = file;
    watermarkImage.value = image;
    error.value = null;
    schedulePreview();
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
  }
}

async function saveImage(): Promise<void> {
  const source = sourceImage.value;
  if (source === null) {
    error.value = t('watermark.errors.noSource');
    return;
  }
  if (settings.mode === 'text' && settings.text.trim() === '') {
    error.value = t('watermark.errors.missingSourceOrContent');
    return;
  }
  if (settings.mode === 'image' && watermarkImage.value === null) {
    error.value = t('watermark.errors.noWatermarkImage');
    return;
  }
  saving.value = true;
  error.value = null;
  try {
    const result = await renderWatermark({
      source,
      watermarkImage: watermarkImage.value,
      settings: { ...settings, textStyle: { ...settings.textStyle } },
      now: new Date(),
      preview: false,
    });
    downloadBlob(result.blob, watermarkedFilename(sourceFile.value?.name ?? '', result.mimeType));
    message.success(t('watermark.messages.imageSaved'));
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
  } finally {
    saving.value = false;
  }
}

function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  try {
    const link = document.createElement('a');
    link.href = url;
    link.download = filename;
    link.click();
  } finally {
    // 给 WebView 下载处理器留出启动时间，再回收 blob URL。
    window.setTimeout(() => {
      URL.revokeObjectURL(url);
    }, 10_000);
  }
}

function copyPreview(): void {
  const canvas = previewCanvasHost.value?.querySelector('canvas');
  if (canvas === null || canvas === undefined) return;
  void (async (): Promise<void> => {
    try {
      const blob = await canvasToPngBlob(canvas);
      const clipboard = Reflect.get(navigator, 'clipboard') as Clipboard | undefined;
      const ClipboardItemConstructor = Reflect.get(globalThis, 'ClipboardItem') as
        typeof ClipboardItem | undefined;
      if (clipboard === undefined || ClipboardItemConstructor === undefined) {
        throw new Error('ui.thisWebviewDoesNotSupportImageClipboardAccess');
      }
      await clipboard.write([new ClipboardItemConstructor({ [blob.type]: blob })]);
      message.success(t('watermark.messages.imageCopied'));
    } catch (caught: unknown) {
      error.value = t(errorMessage(caught));
    }
  })();
}

function canvasToPngBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob === null) {
        reject(new Error('watermark.errors.imageEncodeFailed'));
        return;
      }
      resolve(blob);
    }, 'image/png');
  });
}

function firstImageFile(files: FileList): File | null {
  for (const file of files) {
    if (file.type.startsWith('image/')) return file;
  }
  return null;
}

function errorMessage(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught);
}
</script>

<template>
  <main class="watermark-view" @dragover.prevent @drop.prevent="handleDrop">
    <header class="watermark-view__header">
      <div>
        <h1>{{ t('watermark.title') }}</h1>
        <p>{{ t('watermark.description') }}</p>
      </div>
      <NTag :bordered="false" size="small" type="success">
        {{ t('ui.frontendOnlyLocalProcessing') }}
      </NTag>
    </header>

    <NAlert v-if="error !== null" closable type="error" @close="error = null">
      {{ error }}
    </NAlert>

    <input
      ref="sourceInput"
      class="watermark-view__file-input"
      type="file"
      :accept="SUPPORTED_IMAGE_TYPES.join(',')"
      @change="handleSourceInput"
    />
    <input
      ref="watermarkInput"
      class="watermark-view__file-input"
      type="file"
      :accept="SUPPORTED_IMAGE_TYPES.join(',')"
      @change="handleWatermarkInput"
    />

    <section class="watermark-view__workspace">
      <NCard :bordered="false" class="watermark-view__preview-card" content-style="height: 100%">
        <button
          v-if="sourceImage === null"
          class="watermark-view__empty"
          type="button"
          @click="openSourcePicker"
        >
          <NEmpty :description="t('watermark.empty.chooseDropOrPaste')" />
        </button>
        <div
          v-else
          ref="previewCanvasHost"
          class="watermark-view__preview"
          :aria-label="t('watermark.preview.alt')"
          role="img"
        />
      </NCard>

      <NCard :bordered="false" :title="t('watermark.settings.title')" class="watermark-view__panel">
        <WatermarkSettingsPanel
          :watermark-file-name="watermarkFile?.name ?? null"
          :can-save="canSave"
          :saving="saving"
          :copy-enabled="hasPreview"
          @choose-source="openSourcePicker"
          @choose-watermark="openWatermarkPicker"
          @save="saveImage"
          @copy="copyPreview"
        />
      </NCard>
    </section>

    <footer class="open-source-attribution">
      <span>{{ t('opensource.featureInspiredBy') }}</span>
      <a :href="WATERMARKER_PROJECT_URL" target="_blank" rel="noopener noreferrer">
        TransparentLC/watermarker · {{ t('opensource.openOriginalProject') }}
      </a>
    </footer>
  </main>
</template>

<style scoped lang="scss">
.watermark-view {
  display: grid;
  gap: var(--page-gap);
  grid-template-rows: auto auto minmax(0, 1fr) auto;
  height: var(--app-viewport-height);
  padding: var(--page-padding);

  &__header {
    align-items: center;
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    justify-content: space-between;

    h1,
    p {
      margin: 0;
    }

    p {
      color: var(--muted-color);
      margin-top: 0.25rem;
    }
  }

  &__file-input {
    display: none;
  }

  &__workspace {
    display: grid;
    gap: 1rem;
    grid-template-columns: minmax(0, 1fr) minmax(19rem, 22rem);
    min-height: 0;
  }

  &__preview-card {
    min-height: 0;

    :deep(.n-card__content) {
      display: grid;
      min-height: 0;
    }
  }

  &__empty {
    background: transparent;
    border: 0;
    cursor: pointer;
    display: grid;
    min-height: 100%;
    place-items: center;
    width: 100%;
  }

  &__preview {
    background: repeating-conic-gradient(var(--checker-color-a) 0 25%, var(--checker-color-b) 0 50%)
      50% / 1rem 1rem;
    border-radius: 0.5rem;
    display: grid;
    min-height: 0;
    overflow: hidden;
    place-items: center;

    // 预览画布由脚本插入（renderPreview），保持与容器等比缩放。
    :deep(canvas) {
      display: block;
      max-height: 100%;
      max-width: 100%;
      object-fit: contain;
    }
  }

  &__panel {
    min-height: 0;
    overflow: hidden;

    :deep(.n-card__content) {
      max-height: 100%;
      overflow: auto;
    }
  }
}

@media (width <= 900px) {
  .watermark-view {
    &__header {
      align-items: flex-start;
      flex-direction: column;
      gap: 0.75rem;
    }

    &__workspace {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(14rem, 1fr) auto;
    }
  }
}
</style>
