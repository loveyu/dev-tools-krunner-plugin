<script setup lang="ts">
import type { CSSProperties } from 'vue';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import {
  NAlert,
  NButton,
  NCard,
  NEmpty,
  NInput,
  NInputNumber,
  NSelect,
  NSpin,
  NTag,
  useMessage,
} from 'naive-ui';

import { postRequest } from '../ipc/bridge';
import { executeOcr } from '../ipc/native-media';
import { firstImageFile, prepareImage, SUPPORTED_IMAGE_TYPES } from '../tools/media/image';
import type { OcrCapability, OcrResult, OcrWord, PreparedImage } from '../tools/media/types';

defineOptions({ name: 'OcrView' });

const props = defineProps<{
  readonly capability: OcrCapability;
}>();

const message = useMessage();
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
    options.unshift({ label: '简体中文 + 英文', value: 'chi_sim+eng' });
  }
  return options;
});
const psmOptions = [
  { label: '自动页面分割', value: 3 },
  { label: '单列文本', value: 4 },
  { label: '统一文本块', value: 6 },
  { label: '稀疏文本', value: 11 },
  { label: '稀疏文本（含方向）', value: 12 },
  { label: '单行文本', value: 13 },
];
const versionLabel = computed(() => props.capability.version ?? 'Tesseract 未安装');

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
  if (value === 'chi_sim') return '简体中文';
  if (value === 'eng') return '英文';
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
    message.warning('请先选择或粘贴图片');
    return;
  }
  if (!props.capability.available) {
    message.error('当前系统未提供 Tesseract OCR');
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
    error.value = caught instanceof Error ? caught.message : String(caught);
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
    message.success('已复制识别文字');
  } else {
    message.error('当前环境未提供剪贴板 IPC');
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
        <h1>OCR 文字识别</h1>
        <p>图片仅交给本机 Tesseract，支持选择、拖放和从剪贴板粘贴</p>
      </div>
      <NTag :bordered="false" size="small">{{ versionLabel }}</NTag>
    </header>

    <NAlert v-if="!capability.available" type="warning">
      本机缺少 OCR 能力。Debian 13 可安装：tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim
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
      <NButton @click="openFilePicker">选择图片</NButton>
      <NSelect v-model:value="language" :options="languageOptions" placeholder="识别语言" />
      <NSelect v-model:value="pageSegmentationMode" :options="psmOptions" />
      <NInputNumber
        v-model:value="minimumConfidence"
        :max="100"
        :min="0"
        placeholder="最低置信度"
      />
      <NButton
        :disabled="selectedImage === null || !capability.available"
        :loading="busy"
        type="primary"
        @click="recognize"
      >
        开始识别
      </NButton>
    </section>

    <section class="media-view__panels">
      <NCard class="media-view__panel" title="图片预览" :bordered="false">
        <NSpin :show="busy">
          <div
            class="media-view__drop-zone"
            @click="openFilePicker"
            @dragover.prevent
            @drop.prevent="handleDrop"
          >
            <div v-if="previewUrl !== null" class="media-view__image-wrap">
              <img :src="previewUrl" alt="待识别图片" @load="updateImageDimensions" />
              <span
                v-for="(word, index) in result?.words ?? []"
                :key="`${String(index)}-${word.text}`"
                class="media-view__word-box"
                :style="wordStyle(word)"
                :title="`${word.text} · ${word.confidence.toFixed(1)}%`"
              />
            </div>
            <NEmpty v-else description="拖放图片，或按 Ctrl+V 粘贴图片" />
          </div>
        </NSpin>
      </NCard>

      <NCard class="media-view__panel" :bordered="false">
        <template #header>
          <div class="media-view__result-header">
            <strong>识别结果</strong>
            <NTag v-if="result !== null" :bordered="false" size="small">
              {{ result.words.length }} 词 · 平均 {{ result.averageConfidence.toFixed(1) }}%
            </NTag>
            <NButton
              :disabled="result?.fullText === undefined || result.fullText === ''"
              @click="copyText"
            >
              复制
            </NButton>
          </div>
        </template>
        <NInput
          :value="result?.fullText ?? ''"
          :autosize="{ minRows: 20, maxRows: 30 }"
          class="media-view__output"
          placeholder="识别文字会显示在这里"
          readonly
          type="textarea"
        />
      </NCard>
    </section>
  </main>
</template>

<style scoped lang="scss">
.media-view {
  display: grid;
  gap: 1rem;
  min-height: 100%;
  padding: 1.25rem;

  &__header,
  &__toolbar,
  &__result-header {
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
      margin-top: 0.25rem;
    }
  }

  &__toolbar {
    flex-wrap: wrap;

    :deep(.n-select) {
      min-width: 11rem;
    }

    :deep(.n-input-number) {
      width: 10rem;
    }
  }

  &__file-input {
    display: none;
  }

  &__panels {
    display: grid;
    gap: 1rem;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.9fr);
  }

  &__panel {
    background: var(--panel-color);
  }

  &__drop-zone {
    cursor: pointer;
    display: grid;
    min-height: 31rem;
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
    justify-content: space-between;
  }

  &__output {
    font-family: var(--font-code);
  }
}

@media (width <= 920px) {
  .media-view {
    &__header,
    &__toolbar {
      align-items: stretch;
      flex-direction: column;
    }

    &__panels {
      grid-template-columns: 1fr;
    }

    &__toolbar {
      :deep(.n-input-number),
      :deep(.n-select) {
        min-width: 0;
        width: 100%;
      }
    }
  }
}
</style>
