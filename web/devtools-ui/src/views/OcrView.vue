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
  NSpin,
  NTag,
  useMessage,
} from 'naive-ui';

import CodeEditor from '../components/CodeEditor.vue';
import { postRequest } from '../ipc/bridge';
import { useI18n } from '../i18n/runtime';
import { executeOcr } from '../ipc/native-media';
import { firstImageFile, prepareImage, SUPPORTED_IMAGE_TYPES } from '../tools/media/image';
import type { OcrCapability, OcrResult, OcrWord, PreparedImage } from '../tools/media/types';

defineOptions({ name: 'OcrView' });

const props = defineProps<{
  readonly capability: OcrCapability;
}>();

const message = useMessage();
const { t } = useI18n();
const fileInput = ref<HTMLInputElement | null>(null);
const selectedImage = ref<PreparedImage | null>(null);
const previewUrl = ref<string | null>(null);
const imageWidth = ref<number>(0);
const imageHeight = ref<number>(0);
const language = ref<string>(defaultLanguage(props.capability.languages));
const pageSegmentationMode = ref<number | null>(3);
const minimumConfidence = ref<number | null>(40);
const busy = ref<boolean>(false);
const error = ref<string | null>(null);
const result = ref<OcrResult | null>(null);

const languageOptions = computed(() => {
  const options = props.capability.languages
    .filter((item) => item !== 'osd')
    .map((value) => ({ label: languageLabel(value), value }));
  if (
    props.capability.languages.includes('eng') &&
    props.capability.languages.includes('chi_sim')
  ) {
    options.unshift({ label: t('ui.simplifiedChineseEnglish'), value: 'chi_sim+eng' });
  }
  return options;
});
const psmOptions = computed(() => [
  { label: t('ui.automaticPageSegmentation'), value: 3 },
  { label: t('ui.singleTextColumn'), value: 4 },
  { label: t('ui.uniformTextBlock'), value: 6 },
  { label: t('ui.sparseText'), value: 11 },
  { label: t('ui.sparseTextWithOrientation'), value: 12 },
  { label: t('ui.singleTextLine'), value: 13 },
]);
const versionLabel = computed(() => props.capability.version ?? t('ui.tesseractIsNotInstalled'));

onMounted(() => {
  window.addEventListener('paste', handlePaste);
});
onBeforeUnmount(() => {
  window.removeEventListener('paste', handlePaste);
  releasePreview();
});

function defaultLanguage(languages: readonly string[]): string {
  if (languages.includes('eng') && languages.includes('chi_sim')) return 'chi_sim+eng';
  if (languages.includes('chi_sim')) return 'chi_sim';
  if (languages.includes('eng')) return 'eng';
  return languages.find((item) => item !== 'osd') ?? 'eng';
}

function languageLabel(value: string): string {
  if (value === 'chi_sim') return t('ui.simplifiedChinese');
  if (value === 'eng') return t('ui.english2');
  return value;
}

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
  busy.value = true;
  error.value = null;
  try {
    const prepared = await prepareImage(file);
    releasePreview();
    previewUrl.value = URL.createObjectURL(file);
    selectedImage.value = prepared;
    result.value = null;
    imageWidth.value = 0;
    imageHeight.value = 0;
  } catch (caught: unknown) {
    error.value = caught instanceof Error ? caught.message : String(caught);
  } finally {
    busy.value = false;
  }
}

async function recognize(): Promise<void> {
  if (selectedImage.value === null) {
    message.warning(t('ui.chooseOrPasteAnImageFirst'));
    return;
  }
  if (!props.capability.available) {
    message.error(t('ui.tesseractOcrIsUnavailableOnThisSystem'));
    return;
  }
  busy.value = true;
  error.value = null;
  try {
    result.value = await executeOcr({
      ...selectedImage.value,
      operation: 'ocr',
      options: {
        language: language.value,
        pageSegmentationMode: pageSegmentationMode.value ?? 3,
        minimumConfidence: minimumConfidence.value ?? 0,
      },
    });
  } catch (caught: unknown) {
    error.value = t(caught instanceof Error ? caught.message : String(caught));
  } finally {
    busy.value = false;
  }
}

function updateImageDimensions(event: Event): void {
  const image = event.target;
  if (!(image instanceof HTMLImageElement)) return;
  imageWidth.value = image.naturalWidth;
  imageHeight.value = image.naturalHeight;
}

function wordStyle(word: OcrWord): CSSProperties {
  if (imageWidth.value <= 0 || imageHeight.value <= 0) return {};
  return {
    height: `${String((word.height / imageHeight.value) * 100)}%`,
    left: `${String((word.left / imageWidth.value) * 100)}%`,
    top: `${String((word.top / imageHeight.value) * 100)}%`,
    width: `${String((word.width / imageWidth.value) * 100)}%`,
  };
}

function copyText(): void {
  if (result.value === null || result.value.fullText === '') return;
  if (postRequest({ type: 'clipboardWrite', text: result.value.fullText })) {
    message.success(t('ui.recognizedTextCopied'));
  } else {
    message.error(t('ui.clipboardIpcIsUnavailable'));
  }
}

function releasePreview(): void {
  if (previewUrl.value !== null) URL.revokeObjectURL(previewUrl.value);
  previewUrl.value = null;
}
</script>

<template>
  <main class="media-view">
    <header class="media-view__header">
      <div>
        <h1>{{ t('ui.ocrTextRecognition') }}</h1>
        <p>{{ t('ui.imagesStayOnThisMachineAndAreProcessedByTesseract') }}</p>
      </div>
      <NTag :bordered="false" size="small">{{ versionLabel }}</NTag>
    </header>

    <NAlert v-if="!capability.available" type="warning">
      {{ t('ui.ocrSupportIsMissingOnDebian13Install') }}tesseract-ocr tesseract-ocr-eng
      tesseract-ocr-chi-sim
    </NAlert>
    <NAlert v-if="error !== null" closable type="error" @close="error = null">
      {{ error }}
    </NAlert>

    <section class="media-view__toolbar">
      <input
        ref="fileInput"
        class="media-view__file-input"
        type="file"
        :accept="SUPPORTED_IMAGE_TYPES.join(',')"
        @change="handleFileInput"
      />
      <NButton @click="openFilePicker">{{ t('ui.chooseImage') }}</NButton>
      <NSelect
        v-model:value="language"
        :options="languageOptions"
        :placeholder="t('ui.recognitionLanguage')"
      />
      <NSelect v-model:value="pageSegmentationMode" :options="psmOptions" />
      <NInputNumber
        v-model:value="minimumConfidence"
        :max="100"
        :min="0"
        :placeholder="t('ui.minimumConfidence')"
      />
      <NButton
        :disabled="selectedImage === null || !capability.available"
        :loading="busy"
        type="primary"
        @click="recognize"
      >
        {{ t('ui.recognize') }}
      </NButton>
    </section>

    <section class="media-view__panels">
      <NCard class="media-view__panel" :title="t('ui.imagePreview')" :bordered="false">
        <NSpin :show="busy">
          <div
            class="media-view__drop-zone"
            @click="openFilePicker"
            @dragover.prevent
            @drop.prevent="handleDrop"
          >
            <div v-if="previewUrl !== null" class="media-view__image-wrap">
              <img
                :src="previewUrl"
                :alt="t('ui.imageToRecognize2')"
                @load="updateImageDimensions"
              />
              <span
                v-for="(word, index) in result?.words ?? []"
                :key="`${String(index)}-${word.text}`"
                class="media-view__word-box"
                :style="wordStyle(word)"
                :title="`${word.text} · ${word.confidence.toFixed(1)}%`"
              />
            </div>
            <NEmpty v-else :description="t('ui.dropAnImageOrPressCtrlVToPaste')" />
          </div>
        </NSpin>
      </NCard>

      <NCard class="media-view__panel" :bordered="false">
        <template #header>
          <div class="media-view__result-header">
            <strong>{{ t('ui.recognitionResult') }}</strong>
            <NTag v-if="result !== null" :bordered="false" size="small">
              {{
                t('ui.countWordsConfidenceAverage', {
                  count: result.words.length,
                  confidence: result.averageConfidence.toFixed(1),
                })
              }}
            </NTag>
            <NButton
              :disabled="result?.fullText === undefined || result.fullText === ''"
              @click="copyText"
            >
              {{ t('ui.copy') }}
            </NButton>
          </div>
        </template>
        <CodeEditor
          :model-value="result?.fullText ?? ''"
          class="media-view__output"
          min-height="clamp(18rem, 56vh, 31rem)"
          :placeholder="t('ui.recognizedTextAppearsHere')"
          readonly
        />
      </NCard>
    </section>
  </main>
</template>

<style scoped lang="scss">
.media-view {
  display: flex;
  flex-direction: column;
  gap: var(--page-gap);
  height: var(--app-viewport-height);
  min-height: 0;
  overflow: auto;
  padding: var(--page-padding);

  &__header,
  &__toolbar,
  &__result-header {
    align-items: center;
    display: flex;
    flex-shrink: 0;
    gap: 0.75rem;
  }

  &__header {
    flex-wrap: wrap;
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

  &__toolbar {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));

    :deep(.n-select) {
      min-width: 0;
    }

    :deep(.n-input-number) {
      width: 100%;
    }
  }

  &__file-input {
    display: none;
  }

  &__panels {
    display: grid;
    flex: 1 1 auto;
    gap: 1rem;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.9fr);
    grid-template-rows: minmax(clamp(18rem, 56vh, 31rem), 1fr);
    min-height: 0;
  }

  &__panel {
    background: var(--panel-color);
    display: flex;
    flex-direction: column;
    min-width: 0;

    :deep(.n-card-content) {
      display: flex;
      flex: 1 1 auto;
      flex-direction: column;
      min-height: 0;
    }

    :deep(.n-spin-container),
    :deep(.n-spin-content) {
      display: flex;
      flex: 1 1 auto;
      flex-direction: column;
      min-height: 0;
    }
  }

  &__drop-zone {
    cursor: pointer;
    display: grid;
    flex: 1 1 auto;
    min-height: clamp(18rem, 56vh, 31rem);
    place-items: center;
  }

  &__image-wrap {
    justify-self: center;
    max-height: 66vh;
    max-width: 100%;
    position: relative;
    width: fit-content;

    img {
      display: block;
      max-height: 66vh;
      max-width: 100%;
    }
  }

  &__word-box {
    border: 1px solid rgb(37 99 235 / 80%);
    pointer-events: none;
    position: absolute;
  }

  &__result-header {
    flex-wrap: wrap;
    justify-content: space-between;
  }

  &__output {
    flex: 1 1 auto;
  }
}

@media (width <= 920px) {
  .media-view {
    &__panels {
      // 窄屏堆叠时退出行约束，每个面板保持可用高度并由页面滚动。
      grid-template-columns: 1fr;
      grid-template-rows: none;
    }
  }
}

@media (width <= 600px) {
  .media-view {
    &__header {
      align-items: flex-start;
      flex-direction: column;
    }

    &__toolbar {
      grid-template-columns: 1fr;
    }
  }
}
</style>
