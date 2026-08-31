<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import {
  NAlert,
  NButton,
  NCard,
  NColorPicker,
  NEmpty,
  NInput,
  NInputNumber,
  NRadio,
  NRadioGroup,
  NSelect,
  NSlider,
  NSpin,
  NTag,
  useMessage,
} from 'naive-ui';

import { useI18n } from '../i18n/runtime';
import { validateCompressionFileMetadata } from '../tools/image-compression/model';
import { COMPRESSION_IMAGE_TYPES, OUTPUT_IMAGE_TYPES } from '../tools/image-compression/types';
import type { OutputImageType } from '../tools/image-compression/types';
import { firstImageFile } from '../tools/media/image';
import { WATERMARKER_PROJECT_URL, watermarkedFilename } from '../tools/watermark/model';
import { renderWatermark } from '../tools/watermark/renderer';
import type { WatermarkContent, WatermarkRenderResult } from '../tools/watermark/renderer';

defineOptions({ name: 'WatermarkView' });

type WatermarkMode = 'image' | 'text';

const message = useMessage();
const { t } = useI18n();
const sourceInput = ref<HTMLInputElement | null>(null);
const watermarkInput = ref<HTMLInputElement | null>(null);
const sourceFile = ref<File | null>(null);
const watermarkFile = ref<File | null>(null);
const sourceUrl = ref<string | null>(null);
const outputUrl = ref<string | null>(null);
const outputResult = ref<WatermarkRenderResult | null>(null);
const mode = ref<WatermarkMode>('text');
const text = ref<string>('仅供办理业务使用');
const color = ref<string>('#ffffff');
const fontSize = ref<number | null>(36);
const imageWidth = ref<number | null>(180);
const opacity = ref<number>(28);
const angle = ref<number>(-24);
const horizontalGap = ref<number | null>(120);
const verticalGap = ref<number | null>(90);
const outputType = ref<OutputImageType>('image/png');
const quality = ref<number>(92);
const busy = ref<boolean>(false);
const error = ref<string | null>(null);

const outputTypeOptions = OUTPUT_IMAGE_TYPES.map((value) => ({
  label: value === 'image/jpeg' ? 'JPEG' : value === 'image/webp' ? 'WebP' : 'PNG',
  value,
}));
const canRender = computed(
  () =>
    sourceFile.value !== null &&
    (mode.value === 'text' ? text.value.trim() !== '' : watermarkFile.value !== null),
);

onMounted(() => {
  window.addEventListener('paste', handlePaste);
});
onBeforeUnmount(() => {
  window.removeEventListener('paste', handlePaste);
  releaseSourceUrl();
  releaseOutputUrl();
});

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
  if (file !== null) selectSource(file);
  input.value = '';
}

function handleWatermarkInput(event: Event): void {
  const input = event.target;
  if (!(input instanceof HTMLInputElement) || input.files === null) return;
  const file = firstImageFile(input.files);
  if (file !== null) selectWatermark(file);
  input.value = '';
}

function handleDrop(event: DragEvent): void {
  const file = event.dataTransfer === null ? null : firstImageFile(event.dataTransfer.files);
  if (file !== null) selectSource(file);
}

function handlePaste(event: ClipboardEvent): void {
  if (event.clipboardData === null) return;
  const file = firstImageFile(event.clipboardData.files);
  if (file === null) return;
  event.preventDefault();
  selectSource(file);
}

function selectSource(file: File): void {
  const validationError = validateCompressionFileMetadata(file.type, file.size);
  if (validationError !== null) {
    error.value = validationError;
    return;
  }
  releaseSourceUrl();
  releaseOutputUrl();
  sourceFile.value = file;
  sourceUrl.value = URL.createObjectURL(file);
  outputResult.value = null;
  error.value = null;
}

function selectWatermark(file: File): void {
  const validationError = validateCompressionFileMetadata(file.type, file.size);
  if (validationError !== null) {
    error.value = validationError;
    return;
  }
  watermarkFile.value = file;
  error.value = null;
}

async function applyWatermark(): Promise<void> {
  const source = sourceFile.value;
  const content = watermarkContent();
  if (source === null || content === null) {
    error.value = t('watermark.errors.missingSourceOrContent');
    return;
  }
  const gapX = horizontalGap.value;
  const gapY = verticalGap.value;
  if (gapX === null || gapY === null || gapX < 8 || gapY < 8) {
    error.value = t('watermark.errors.invalidSpacing');
    return;
  }

  busy.value = true;
  error.value = null;
  try {
    const result = await renderWatermark(source, content, {
      angle: angle.value,
      horizontalGap: gapX,
      mimeType: outputType.value,
      opacityPercent: opacity.value,
      qualityPercent: quality.value,
      verticalGap: gapY,
    });
    releaseOutputUrl();
    outputResult.value = result;
    outputUrl.value = URL.createObjectURL(result.blob);
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
  } finally {
    busy.value = false;
  }
}

function watermarkContent(): WatermarkContent | null {
  if (mode.value === 'text') {
    const value = text.value.trim();
    if (value === '') return null;
    return {
      type: 'text',
      color: color.value,
      fontSize: fontSize.value ?? 36,
      text: value,
    };
  }
  const file = watermarkFile.value;
  return file === null ? null : { type: 'image', file, width: imageWidth.value ?? 180 };
}

function downloadOutput(): void {
  const source = sourceFile.value;
  const result = outputResult.value;
  const url = outputUrl.value;
  if (source === null || result === null || url === null) return;
  const link = document.createElement('a');
  link.href = url;
  link.download = watermarkedFilename(source.name, result.mimeType);
  link.click();
}

function copyOutput(): void {
  const result = outputResult.value;
  const clipboard = Reflect.get(navigator, 'clipboard') as Clipboard | undefined;
  const ClipboardItemConstructor = Reflect.get(globalThis, 'ClipboardItem') as
    typeof ClipboardItem | undefined;
  if (result === null || clipboard === undefined || ClipboardItemConstructor === undefined) {
    error.value = t('ui.thisWebviewDoesNotSupportImageClipboardAccess');
    return;
  }
  void clipboard.write([new ClipboardItemConstructor({ [result.blob.type]: result.blob })]).then(
    () => message.success(t('watermark.messages.imageCopied')),
    (caught: unknown) => {
      error.value = t('ui.failedToCopyImageError', { error: errorMessage(caught) });
    },
  );
}

function releaseSourceUrl(): void {
  if (sourceUrl.value !== null) URL.revokeObjectURL(sourceUrl.value);
  sourceUrl.value = null;
}

function releaseOutputUrl(): void {
  if (outputUrl.value !== null) URL.revokeObjectURL(outputUrl.value);
  outputUrl.value = null;
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
      :accept="COMPRESSION_IMAGE_TYPES.join(',')"
      @change="handleSourceInput"
    />
    <input
      ref="watermarkInput"
      class="watermark-view__file-input"
      type="file"
      :accept="COMPRESSION_IMAGE_TYPES.join(',')"
      @change="handleWatermarkInput"
    />

    <section class="watermark-view__workspace">
      <NCard :bordered="false" :title="t('watermark.settings.title')">
        <div class="watermark-view__form">
          <NButton block @click="openSourcePicker">{{
            t('watermark.actions.chooseSource')
          }}</NButton>
          <NRadioGroup v-model:value="mode" name="watermark-mode">
            <NRadio value="text">{{ t('watermark.mode.text') }}</NRadio>
            <NRadio value="image">{{ t('watermark.mode.image') }}</NRadio>
          </NRadioGroup>

          <template v-if="mode === 'text'">
            <label class="watermark-view__field">
              <span>{{ t('watermark.fields.text') }}</span>
              <NInput v-model:value="text" :maxlength="120" />
            </label>
            <div class="watermark-view__split-fields">
              <label class="watermark-view__field">
                <span>{{ t('watermark.fields.fontSize') }}</span>
                <NInputNumber v-model:value="fontSize" :max="240" :min="8" :precision="0" />
              </label>
              <label class="watermark-view__field">
                <span>{{ t('watermark.fields.color') }}</span>
                <NColorPicker v-model:value="color" :show-alpha="false" />
              </label>
            </div>
          </template>
          <template v-else>
            <NButton block @click="openWatermarkPicker">
              {{ t('watermark.actions.chooseWatermarkImage') }}
            </NButton>
            <p v-if="watermarkFile !== null" class="watermark-view__hint">
              {{ watermarkFile.name }}
            </p>
            <label class="watermark-view__field">
              <span>{{ t('watermark.fields.imageWidth') }}</span>
              <NInputNumber v-model:value="imageWidth" :max="2000" :min="16" :precision="0" />
            </label>
          </template>

          <label class="watermark-view__field">
            <span>{{ t('watermark.fields.opacity') }} · {{ opacity }}%</span>
            <NSlider v-model:value="opacity" :max="100" :min="1" :step="1" />
          </label>
          <label class="watermark-view__field">
            <span>{{ t('watermark.fields.angle') }} · {{ angle }}°</span>
            <NSlider v-model:value="angle" :max="90" :min="-90" :step="1" />
          </label>
          <div class="watermark-view__split-fields">
            <label class="watermark-view__field">
              <span>{{ t('watermark.fields.horizontalGap') }}</span>
              <NInputNumber v-model:value="horizontalGap" :max="2000" :min="8" :precision="0" />
            </label>
            <label class="watermark-view__field">
              <span>{{ t('watermark.fields.verticalGap') }}</span>
              <NInputNumber v-model:value="verticalGap" :max="2000" :min="8" :precision="0" />
            </label>
          </div>
          <div class="watermark-view__split-fields">
            <label class="watermark-view__field">
              <span>{{ t('ui.outputFormat') }}</span>
              <NSelect v-model:value="outputType" :options="outputTypeOptions" />
            </label>
            <label class="watermark-view__field">
              <span>{{ t('ui.quality') }} · {{ quality }}%</span>
              <NSlider v-model:value="quality" :disabled="outputType === 'image/png'" />
            </label>
          </div>
          <NButton
            block
            :disabled="!canRender"
            :loading="busy"
            type="primary"
            @click="applyWatermark"
          >
            {{ t('watermark.actions.apply') }}
          </NButton>
          <div class="watermark-view__actions">
            <NButton block :disabled="outputResult === null" @click="copyOutput">
              {{ t('ui.copyPng') }}
            </NButton>
            <NButton block :disabled="outputResult === null" @click="downloadOutput">
              {{ t('watermark.actions.download') }}
            </NButton>
          </div>
        </div>
      </NCard>

      <NCard :bordered="false" class="watermark-view__preview-card">
        <NSpin :show="busy">
          <button
            v-if="sourceUrl === null"
            class="watermark-view__empty"
            type="button"
            @click="openSourcePicker"
          >
            <NEmpty :description="t('watermark.empty.chooseDropOrPaste')" />
          </button>
          <div v-else class="watermark-view__preview">
            <img :src="outputUrl ?? sourceUrl" :alt="t('watermark.preview.alt')" />
          </div>
        </NSpin>
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
  gap: 1rem;
  min-height: 100vh;
  padding: 1.25rem;

  &__header {
    align-items: center;
    display: flex;
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
    grid-template-columns: minmax(19rem, 23rem) minmax(0, 1fr);
  }

  &__form,
  &__field {
    display: grid;
    gap: 0.6rem;
  }

  &__field,
  &__hint {
    color: var(--muted-color);
    font-size: 0.82rem;
  }

  &__hint {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  &__split-fields,
  &__actions {
    display: grid;
    gap: 0.75rem;
    grid-template-columns: 1fr 1fr;
  }

  &__preview-card {
    min-height: 34rem;
  }

  &__empty {
    background: transparent;
    border: 0;
    cursor: pointer;
    display: grid;
    min-height: 32rem;
    place-items: center;
    width: 100%;
  }

  &__preview {
    background: repeating-conic-gradient(var(--checker-color-a) 0 25%, var(--checker-color-b) 0 50%)
      50% / 1rem 1rem;
    border-radius: 0.5rem;
    display: grid;
    min-height: 32rem;
    overflow: hidden;
    place-items: center;

    img {
      display: block;
      max-height: 70vh;
      max-width: 100%;
      object-fit: contain;
    }
  }
}

@media (width <= 860px) {
  .watermark-view {
    &__header {
      align-items: flex-start;
      flex-direction: column;
      gap: 0.75rem;
    }

    &__workspace {
      grid-template-columns: 1fr;
    }
  }
}
</style>
