<script setup lang="ts">
import type { CSSProperties } from 'vue';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import {
  NAlert,
  NButton,
  NCard,
  NEmpty,
  NInputNumber,
  NSelect,
  NSlider,
  NSpin,
  NStatistic,
  NTag,
  useMessage,
} from 'naive-ui';

import { useI18n } from '../i18n/runtime';
import { firstImageFile } from '../tools/media/image';
import { compressImage, inspectImage } from '../tools/image-compression/compressor';
import {
  MAX_OUTPUT_DIMENSION,
  defaultOutputType,
  formatFileSize,
  initialTargetDimensions,
  outputFilename,
  validateCompressionFileMetadata,
} from '../tools/image-compression/model';
import type {
  CompressionResult,
  ImageDimensions,
  OutputImageType,
} from '../tools/image-compression/types';
import { COMPRESSION_IMAGE_TYPES } from '../tools/image-compression/types';

defineOptions({ name: 'ImageCompressionView' });

const message = useMessage();
const { t } = useI18n();
const fileInput = ref<HTMLInputElement | null>(null);
const selectedFile = ref<File | null>(null);
const sourceDimensions = ref<ImageDimensions | null>(null);
const outputResult = ref<CompressionResult | null>(null);
const originalUrl = ref<string | null>(null);
const outputUrl = ref<string | null>(null);
const outputType = ref<OutputImageType>('image/webp');
const qualityPercent = ref<number>(82);
const maxWidth = ref<number | null>(null);
const maxHeight = ref<number | null>(null);
const comparisonPosition = ref<number>(50);
const busy = ref<boolean>(false);
const error = ref<string | null>(null);

const outputTypeOptions = computed<{ readonly label: string; readonly value: OutputImageType }[]>(
  () => [
    { label: 'WebP', value: 'image/webp' },
    { label: 'JPEG', value: 'image/jpeg' },
    { label: `PNG（${t('ui.lossless')}）`, value: 'image/png' },
  ],
);
const qualityDisabled = computed(() => outputType.value === 'image/png');
const comparisonClipStyle = computed<CSSProperties>(() => ({
  clipPath: `inset(0 ${String(100 - comparisonPosition.value)}% 0 0)`,
}));
const comparisonDividerStyle = computed<CSSProperties>(() => ({
  left: `${String(comparisonPosition.value)}%`,
}));
const originalSizeLabel = computed(() =>
  selectedFile.value === null ? '—' : formatFileSize(selectedFile.value.size),
);
const outputSizeLabel = computed(() =>
  outputResult.value === null ? '—' : formatFileSize(outputResult.value.blob.size),
);
const deltaLabel = computed(() => {
  if (selectedFile.value === null || outputResult.value === null) return '—';
  const originalBytes = selectedFile.value.size;
  if (originalBytes <= 0) return '—';
  const outputBytes = outputResult.value.blob.size;
  const percentage = Math.abs((1 - outputBytes / originalBytes) * 100).toFixed(1);
  return t(outputBytes <= originalBytes ? 'ui.percentageSmaller' : 'ui.percentageLarger', {
    percentage,
  });
});
const outputDimensionsLabel = computed(() =>
  outputResult.value === null
    ? '—'
    : `${String(outputResult.value.width)} × ${String(outputResult.value.height)}`,
);

onMounted(() => {
  window.addEventListener('paste', handlePaste);
});

onBeforeUnmount(() => {
  window.removeEventListener('paste', handlePaste);
  releaseOriginalUrl();
  releaseOutputUrl();
});

function openFilePicker(): void {
  fileInput.value?.click();
}

function handleFileInput(event: Event): void {
  const input = event.target;
  if (!(input instanceof HTMLInputElement) || input.files === null) return;
  const file = firstImageFile(input.files);
  if (file !== null) void selectFile(file);
  input.value = '';
}

function handlePaste(event: ClipboardEvent): void {
  if (event.clipboardData === null) return;
  const file = firstImageFile(event.clipboardData.files);
  if (file === null) return;
  event.preventDefault();
  void selectFile(file);
}

function handleDrop(event: DragEvent): void {
  const file = event.dataTransfer === null ? null : firstImageFile(event.dataTransfer.files);
  if (file !== null) void selectFile(file);
}

async function selectFile(file: File): Promise<void> {
  error.value = null;
  const validationError = validateCompressionFileMetadata(file.type, file.size);
  if (validationError !== null) {
    error.value = t(validationError);
    return;
  }

  busy.value = true;
  try {
    const dimensions = await inspectImage(file);
    const target = initialTargetDimensions(dimensions.width, dimensions.height);
    releaseOriginalUrl();
    releaseOutputUrl();
    selectedFile.value = file;
    sourceDimensions.value = dimensions;
    originalUrl.value = URL.createObjectURL(file);
    outputType.value = defaultOutputType(file.type);
    maxWidth.value = target.width;
    maxHeight.value = target.height;
    outputResult.value = null;
  } catch (caught: unknown) {
    error.value = t(errorMessage(caught));
    return;
  } finally {
    busy.value = false;
  }
  await runCompression();
}

async function runCompression(): Promise<void> {
  if (selectedFile.value === null || maxWidth.value === null || maxHeight.value === null) {
    message.warning(t('ui.chooseAnImageAndSetValidOutputDimensionsFirst'));
    return;
  }

  busy.value = true;
  error.value = null;
  try {
    const result = await compressImage(selectedFile.value, {
      mimeType: outputType.value,
      qualityPercent: qualityPercent.value,
      maxWidth: maxWidth.value,
      maxHeight: maxHeight.value,
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

function downloadOutput(): void {
  if (selectedFile.value === null || outputResult.value === null || outputUrl.value === null)
    return;
  const link = document.createElement('a');
  link.href = outputUrl.value;
  link.download = outputFilename(selectedFile.value.name, outputResult.value.mimeType);
  link.click();
}

function releaseOriginalUrl(): void {
  if (originalUrl.value !== null) URL.revokeObjectURL(originalUrl.value);
  originalUrl.value = null;
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
  <main class="image-compression">
    <header class="image-compression__header">
      <div>
        <h1>{{ t('ui.imageCompression') }}</h1>
        <p>{{ t('ui.squooshInspiredLocalProcessingAndComparisonImagesAreNeverSent') }}</p>
      </div>
      <NTag :bordered="false" size="small" type="success">
        {{ t('ui.frontendOnlyLocalProcessing') }}
      </NTag>
    </header>

    <NAlert v-if="error !== null" closable type="error" @close="error = null">
      {{ error }}
    </NAlert>
    <NAlert v-if="selectedFile?.type === 'image/gif'" type="warning">
      {{ t('ui.gifInputIsCompressedAsAStaticImageAnimationIs') }}
    </NAlert>

    <input
      ref="fileInput"
      class="image-compression__file-input"
      type="file"
      :accept="COMPRESSION_IMAGE_TYPES.join(',')"
      @change="handleFileInput"
    />

    <section class="image-compression__workspace">
      <NCard
        class="image-compression__settings"
        :title="t('ui.compressionSettings')"
        :bordered="false"
      >
        <div class="image-compression__form">
          <NButton block @click="openFilePicker">{{ t('ui.chooseImage') }}</NButton>

          <label class="image-compression__field">
            <span>{{ t('ui.outputFormat') }}</span>
            <NSelect v-model:value="outputType" :options="outputTypeOptions" />
          </label>

          <label class="image-compression__field">
            <span
              >{{ t('ui.quality') }}
              {{
                qualityDisabled ? `（${t('ui.notApplicableToPng')}）` : `${String(qualityPercent)}%`
              }}</span
            >
            <NSlider
              v-model:value="qualityPercent"
              :disabled="qualityDisabled"
              :max="100"
              :min="1"
              :step="1"
            />
          </label>

          <div class="image-compression__dimensions">
            <label class="image-compression__field">
              <span>{{ t('ui.maximumWidth') }}</span>
              <NInputNumber
                v-model:value="maxWidth"
                :max="MAX_OUTPUT_DIMENSION"
                :min="1"
                :precision="0"
              />
            </label>
            <label class="image-compression__field">
              <span>{{ t('ui.maximumHeight') }}</span>
              <NInputNumber
                v-model:value="maxHeight"
                :max="MAX_OUTPUT_DIMENSION"
                :min="1"
                :precision="0"
              />
            </label>
          </div>
          <p class="image-compression__hint">
            {{
              t('ui.fitsWithinTheseBoundsWithoutUpscalingMaximumSideIsSize', {
                size: MAX_OUTPUT_DIMENSION,
              })
            }}
          </p>

          <NButton
            block
            :disabled="selectedFile === null || maxWidth === null || maxHeight === null"
            :loading="busy"
            type="primary"
            @click="runCompression"
          >
            {{ t('ui.applyCompression') }}
          </NButton>
          <NButton block :disabled="outputResult === null" @click="downloadOutput">
            {{ t('ui.downloadCompressedImage') }}
          </NButton>
        </div>
      </NCard>

      <NCard class="image-compression__preview-card" :bordered="false">
        <NSpin :show="busy">
          <div
            v-if="originalUrl === null"
            class="image-compression__drop-zone"
            role="button"
            tabindex="0"
            @click="openFilePicker"
            @dragover.prevent
            @drop.prevent="handleDrop"
            @keydown.enter="openFilePicker"
            @keydown.space.prevent="openFilePicker"
          >
            <NEmpty :description="t('ui.dropClickToChooseOrPressCtrlVToPaste')" />
          </div>

          <div v-else class="image-compression__preview">
            <div class="image-compression__compare-stage">
              <img :src="originalUrl" :alt="t('ui.originalImagePreview')" />
              <img
                v-if="outputUrl !== null"
                class="image-compression__optimized-image"
                :src="outputUrl"
                :style="comparisonClipStyle"
                :alt="t('ui.compressedImagePreview')"
              />
              <span class="image-compression__label image-compression__label--original">
                {{ t('ui.original') }}
              </span>
              <span class="image-compression__label image-compression__label--output">
                {{ t('ui.compressed') }}
              </span>
              <span
                v-if="outputUrl !== null"
                class="image-compression__divider"
                :style="comparisonDividerStyle"
              />
            </div>
            <NSlider v-model:value="comparisonPosition" :max="100" :min="0" :step="1" />
          </div>
        </NSpin>
      </NCard>
    </section>

    <section class="image-compression__stats">
      <NCard :bordered="false">
        <NStatistic :label="t('ui.originalSize')" :value="originalSizeLabel" />
      </NCard>
      <NCard :bordered="false">
        <NStatistic :label="t('ui.compressedSize')" :value="outputSizeLabel" />
      </NCard>
      <NCard :bordered="false">
        <NStatistic :label="t('ui.sizeChange')" :value="deltaLabel" />
      </NCard>
      <NCard :bordered="false">
        <NStatistic :label="t('ui.outputDimensions')" :value="outputDimensionsLabel" />
      </NCard>
    </section>

    <p v-if="sourceDimensions !== null" class="image-compression__source-meta">
      {{ selectedFile?.name }} · {{ t('ui.originalDimensions') }} {{ sourceDimensions.width }} ×
      {{ sourceDimensions.height }}
    </p>
    <footer class="open-source-attribution">
      <span>{{ t('opensource.featureInspiredBy') }}</span>
      <a
        href="https://github.com/GoogleChromeLabs/squoosh"
        target="_blank"
        rel="noopener noreferrer"
      >
        Squoosh · {{ t('opensource.openOriginalProject') }}
      </a>
    </footer>
  </main>
</template>

<style scoped lang="scss">
.image-compression {
  display: grid;
  gap: var(--page-gap);
  height: var(--app-viewport-height);
  min-height: 0;
  overflow: auto;
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
    grid-template-columns: minmax(17rem, 20rem) minmax(0, 1fr);
    min-height: 0;
  }

  &__settings,
  &__preview-card,
  &__stats :deep(.n-card) {
    background: var(--panel-color);
  }

  &__form,
  &__field {
    display: grid;
    gap: 0.65rem;
  }

  &__field {
    color: var(--muted-color);
    font-size: 0.875rem;
  }

  &__dimensions {
    display: grid;
    gap: 0.75rem;
    grid-template-columns: 1fr 1fr;
  }

  &__hint,
  &__source-meta {
    color: var(--muted-color);
    font-size: 0.8rem;
    margin: 0;
  }

  &__drop-zone {
    cursor: pointer;
    display: grid;
    min-height: clamp(18rem, 58vh, 34rem);
    outline: none;
    place-items: center;
  }

  &__preview {
    display: grid;
    gap: 1rem;
  }

  &__compare-stage {
    background: repeating-conic-gradient(var(--checker-color-a) 0 25%, var(--checker-color-b) 0 50%)
      50% / 1rem 1rem;
    border-radius: 0.5rem;
    display: grid;
    height: clamp(18rem, 56vh, 34rem);
    overflow: hidden;
    place-items: center;
    position: relative;

    img {
      display: block;
      height: 100%;
      object-fit: contain;
      width: 100%;
    }
  }

  &__optimized-image {
    height: 100%;
    inset: 0;
    object-fit: contain;
    position: absolute;
    width: 100%;
  }

  &__label {
    background: rgb(15 23 42 / 72%);
    border-radius: 999px;
    color: #fff;
    font-size: 0.75rem;
    padding: 0.25rem 0.55rem;
    position: absolute;
    top: 0.75rem;
    z-index: 2;

    &--original {
      right: 0.75rem;
    }

    &--output {
      left: 0.75rem;
    }
  }

  &__divider {
    background: #fff;
    bottom: 0;
    box-shadow: 0 0 0 1px rgb(15 23 42 / 35%);
    position: absolute;
    top: 0;
    transform: translateX(-1px);
    width: 2px;
    z-index: 2;
  }

  &__stats {
    display: grid;
    gap: 1rem;
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}

@media (width <= 820px) {
  .image-compression {
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

@media (width <= 900px) {
  .image-compression__stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (width <= 520px) {
  .image-compression {
    &__dimensions,
    &__stats {
      grid-template-columns: 1fr;
    }
  }
}
</style>
