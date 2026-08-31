<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { NAlert, NButton, NInputNumber, NSelect, NTag, useMessage } from 'naive-ui';
import ImageEditor from 'tui-image-editor';
import 'tui-image-editor/dist/tui-image-editor.css';

import { useI18n } from '../i18n/runtime';
import { toastEditorLocale, TOAST_EDITOR_THEME } from '../tools/image-editor/config';
import {
  dataUrlToBlob,
  editedImageFilename,
  normalizeExportQuality,
} from '../tools/image-editor/export';
import type { EditorExportFormat } from '../tools/image-editor/export';
import { validateCompressionFileMetadata } from '../tools/image-compression/model';
import { COMPRESSION_IMAGE_TYPES } from '../tools/image-compression/types';
import { firstImageFile } from '../tools/media/image';

defineOptions({ name: 'ImageEditorView' });

type ImageEditorOptions = NonNullable<ConstructorParameters<typeof ImageEditor>[1]>;
type IncludeUiOptions = NonNullable<ImageEditorOptions['includeUI']> & {
  readonly locale: Readonly<Record<string, string>>;
};

const message = useMessage();
const { locale, t } = useI18n();
const editorMount = ref<HTMLDivElement | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const imageLoaded = ref<boolean>(false);
const sourceName = ref<string>('');
const busy = ref<boolean>(false);
const error = ref<string | null>(null);
const exportFormat = ref<EditorExportFormat>('png');
const exportQuality = ref<number | null>(92);
let editor: ImageEditor | null = null;

const exportFormatOptions: { readonly label: string; readonly value: EditorExportFormat }[] = [
  { label: 'PNG', value: 'png' },
  { label: 'JPEG', value: 'jpeg' },
];

onMounted(() => {
  const mount = editorMount.value;
  if (mount === null) {
    error.value = t('ui.theImageEditorContainerIsNotReady');
    return;
  }
  const includeUiOptions: IncludeUiOptions = {
    theme: TOAST_EDITOR_THEME,
    locale: toastEditorLocale(locale.value),
    menu: ['resize', 'crop', 'flip', 'rotate', 'draw', 'shape', 'icon', 'text', 'filter'],
    initMenu: 'crop',
    uiSize: { width: '100%', height: '100%' },
    menuBarPosition: 'bottom',
    usageStatistics: false,
  };
  editor = new ImageEditor(mount, {
    includeUI: includeUiOptions,
    cssMaxWidth: 1400,
    cssMaxHeight: 900,
    usageStatistics: false,
  });
  window.addEventListener('paste', handlePaste);
  window.addEventListener('resize', handleResize);
});

onBeforeUnmount(() => {
  window.removeEventListener('paste', handlePaste);
  window.removeEventListener('resize', handleResize);
  editor?.destroy();
  editor = null;
});

function openFilePicker(): void {
  fileInput.value?.click();
}

function handleFileInput(event: Event): void {
  const input = event.target;
  if (!(input instanceof HTMLInputElement) || input.files === null) return;
  const file = firstImageFile(input.files);
  if (file !== null) void loadFile(file);
  input.value = '';
}

function handlePaste(event: ClipboardEvent): void {
  if (event.clipboardData === null) return;
  const file = firstImageFile(event.clipboardData.files);
  if (file === null) return;
  event.preventDefault();
  void loadFile(file);
}

function handleDrop(event: DragEvent): void {
  const file = event.dataTransfer === null ? null : firstImageFile(event.dataTransfer.files);
  if (file !== null) void loadFile(file);
}

function handleResize(): void {
  if (editor !== null) void editor.ui.resizeEditor({});
}

async function loadFile(file: File): Promise<void> {
  const currentEditor = editor;
  if (currentEditor === null) return;
  const validationError = validateCompressionFileMetadata(file.type, file.size);
  if (validationError !== null) {
    error.value = t(validationError);
    return;
  }

  busy.value = true;
  error.value = null;
  try {
    const dimensions = await currentEditor.loadImageFromFile(file, file.name);
    await currentEditor.ui.resizeEditor({ imageSize: dimensions });
    currentEditor.clearUndoStack();
    currentEditor.clearRedoStack();
    sourceName.value = file.name;
    imageLoaded.value = true;
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
  } finally {
    busy.value = false;
  }
}

function copyEditedImage(): void {
  if (!imageLoaded.value || editor === null) return;
  const clipboard = Reflect.get(navigator, 'clipboard') as Clipboard | undefined;
  const ClipboardItemConstructor = Reflect.get(globalThis, 'ClipboardItem') as
    typeof ClipboardItem | undefined;
  if (clipboard === undefined || ClipboardItemConstructor === undefined) {
    error.value = t('ui.thisWebviewDoesNotSupportImageClipboardAccess');
    return;
  }

  try {
    const blob = dataUrlToBlob(editor.toDataURL({ format: 'png' }));
    const writeResult = clipboard.write([new ClipboardItemConstructor({ 'image/png': blob })]);
    void writeResult.then(
      (): void => {
        message.success(t('ui.pngImageCopied'));
      },
      (caught: unknown): void => {
        error.value = t('ui.failedToCopyImageError', { error: errorMessage(caught) });
      },
    );
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
  }
}

function exportEditedImage(): void {
  if (!imageLoaded.value || editor === null) return;
  try {
    const format = exportFormat.value;
    const blob = dataUrlToBlob(
      editor.toDataURL({
        format,
        quality: normalizeExportQuality(exportQuality.value ?? 92),
      }),
    );
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = editedImageFilename(sourceName.value, format);
    link.click();
    URL.revokeObjectURL(url);
    message.success(t('ui.exportedFormat', { format: format === 'jpeg' ? 'JPEG' : 'PNG' }));
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
  }
}

function errorMessage(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught);
}
</script>

<template>
  <main class="image-editor-view" @dragover.prevent @drop.prevent="handleDrop">
    <header class="image-editor-view__header">
      <div>
        <h1>{{ t('ui.imageEditor') }}</h1>
        <p>TOAST UI Image Editor · {{ t('ui.cropResizeFlipRotateDrawAddShapesAndTextAnd') }}</p>
      </div>
      <NTag :bordered="false" size="small" type="success">
        {{ t('ui.frontendOnlyTelemetryDisabled') }}
      </NTag>
    </header>

    <NAlert v-if="error !== null" closable type="error" @close="error = null">
      {{ error }}
    </NAlert>

    <input
      ref="fileInput"
      class="image-editor-view__file-input"
      type="file"
      :accept="COMPRESSION_IMAGE_TYPES.join(',')"
      @change="handleFileInput"
    />

    <section class="image-editor-view__toolbar">
      <NButton :loading="busy" @click="openFilePicker">{{ t('ui.chooseImage') }}</NButton>
      <NButton :disabled="!imageLoaded" @click="copyEditedImage">{{ t('ui.copyPng') }}</NButton>
      <NSelect
        v-model:value="exportFormat"
        class="image-editor-view__format"
        :options="exportFormatOptions"
      />
      <NInputNumber
        v-model:value="exportQuality"
        class="image-editor-view__quality"
        :disabled="exportFormat === 'png'"
        :max="100"
        :min="1"
        :precision="0"
        :placeholder="t('ui.jpegQuality')"
      />
      <NButton :disabled="!imageLoaded" type="primary" @click="exportEditedImage">
        {{ t('ui.exportImage') }}
      </NButton>
      <span v-if="imageLoaded" class="image-editor-view__filename">{{ sourceName }}</span>
    </section>

    <section class="image-editor-view__editor-shell">
      <div ref="editorMount" class="image-editor-view__mount" />
      <button
        v-if="!imageLoaded"
        class="image-editor-view__empty"
        type="button"
        @click="openFilePicker"
      >
        <strong>{{ t('ui.chooseDropOrPasteAnImageToStartEditing') }}</strong>
        <span>{{ t('ui.supportsPngJpegWebpBmpAndGifUpTo25') }}</span>
      </button>
    </section>
    <footer class="open-source-attribution">
      <span>{{ t('opensource.featureInspiredBy') }}</span>
      <a href="https://github.com/nhn/tui.image-editor" target="_blank" rel="noopener noreferrer">
        TOAST UI Image Editor · {{ t('opensource.openOriginalProject') }}
      </a>
    </footer>
  </main>
</template>

<style scoped lang="scss">
.image-editor-view {
  display: grid;
  gap: 0.75rem;
  height: 100vh;
  min-height: 36rem;
  padding: 1rem;

  &__header,
  &__toolbar {
    align-items: center;
    display: flex;
    gap: 0.75rem;
  }

  &__header {
    justify-content: space-between;

    h1,
    p {
      margin: 0;
    }

    p {
      color: var(--muted-color);
      margin-top: 0.2rem;
    }
  }

  &__file-input {
    display: none;
  }

  &__toolbar {
    flex-wrap: wrap;
  }

  &__format {
    width: 7rem;
  }

  &__quality {
    width: 9rem;
  }

  &__filename {
    color: var(--muted-color);
    margin-left: auto;
    max-width: 20rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  &__editor-shell {
    border: 1px solid var(--border-color);
    border-radius: 0.65rem;
    min-height: 0;
    overflow: hidden;
    position: relative;
  }

  &__mount {
    height: 100%;
    min-height: 31rem;
    width: 100%;

    :deep(.tui-image-editor-header) {
      display: none;
    }

    :deep(.tui-image-editor-main) {
      top: 0;
    }

    :deep(.tui-image-editor-container) {
      font-family: Inter, 'Noto Sans', system-ui, sans-serif;
    }
  }

  &__empty {
    align-items: center;
    background: rgb(15 23 42 / 48%);
    border: 1px dashed rgb(255 255 255 / 45%);
    border-radius: 0.75rem;
    color: #fff;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    inset: 15% 18%;
    justify-content: center;
    position: absolute;
    z-index: 4;

    span {
      color: rgb(255 255 255 / 72%);
      font-size: 0.85rem;
    }
  }
}

@media (width <= 820px) {
  .image-editor-view {
    &__header {
      align-items: flex-start;
      flex-direction: column;
    }

    &__filename {
      margin-left: 0;
      max-width: 100%;
      width: 100%;
    }

    &__empty {
      inset: 12% 8%;
    }
  }
}
</style>
