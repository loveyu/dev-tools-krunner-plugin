<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { toCanvas } from '@bwip-js/browser';
import {
  NAlert,
  NButton,
  NCard,
  NCode,
  NEmpty,
  NInputNumber,
  NSelect,
  NSpin,
  NTabPane,
  NTabs,
  NTag,
  useMessage,
} from 'naive-ui';

import CodeEditor from '../components/CodeEditor.vue';
import { postRequest } from '../ipc/bridge';
import { useI18n } from '../i18n/runtime';
import { executeBarcode } from '../ipc/native-media';
import { buildBarcodeOptions } from '../tools/media/barcode-generator';
import { firstImageFile, prepareImage, SUPPORTED_IMAGE_TYPES } from '../tools/media/image';
import type {
  BarcodeCapability,
  BarcodeFormat,
  BarcodeRecognitionResult,
  PreparedImage,
} from '../tools/media/types';

defineOptions({ name: 'BarcodeStudioView' });

const props = defineProps<{
  readonly capability: BarcodeCapability;
}>();

type TabName = 'generate' | 'recognize';

const message = useMessage();
const { t } = useI18n();
const activeTab = ref<TabName>('recognize');
const fileInput = ref<HTMLInputElement | null>(null);
const selectedImage = ref<PreparedImage | null>(null);
const previewUrl = ref<string | null>(null);
const recognitionResult = ref<BarcodeRecognitionResult | null>(null);
const recognitionBusy = ref<boolean>(false);
const recognitionError = ref<string | null>(null);
const canvas = ref<HTMLCanvasElement | null>(null);
const generationFormat = ref<BarcodeFormat>('qrcode');
const generationText = ref<string>('https://example.com');
const generationScale = ref<number | null>(4);
const generationError = ref<string | null>(null);
const generated = ref<boolean>(false);

const formatOptions = computed<{ readonly label: string; readonly value: BarcodeFormat }[]>(() => [
  { label: t('ui.qrCode'), value: 'qrcode' },
  { label: 'Code 128', value: 'code128' },
  { label: 'Code 39', value: 'code39' },
  { label: 'EAN-13', value: 'ean13' },
]);

onMounted(() => {
  window.addEventListener('paste', handlePaste);
});
onBeforeUnmount(() => {
  window.removeEventListener('paste', handlePaste);
  releasePreview();
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
  if (activeTab.value !== 'recognize' || event.clipboardData === null) return;
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
  recognitionBusy.value = true;
  recognitionError.value = null;
  try {
    const prepared = await prepareImage(file);
    releasePreview();
    previewUrl.value = URL.createObjectURL(file);
    selectedImage.value = prepared;
    recognitionResult.value = null;
    if (props.capability.available) await recognize();
  } catch (caught: unknown) {
    recognitionError.value = caught instanceof Error ? caught.message : String(caught);
  } finally {
    recognitionBusy.value = false;
  }
}

async function recognize(): Promise<void> {
  if (selectedImage.value === null) {
    message.warning(t('ui.chooseOrPasteAnImageFirst'));
    return;
  }
  if (!props.capability.available) {
    message.error(t('ui.zbarBarcodeRecognitionIsUnavailableOnThisSystem'));
    return;
  }
  recognitionBusy.value = true;
  recognitionError.value = null;
  try {
    recognitionResult.value = await executeBarcode({
      ...selectedImage.value,
      operation: 'barcode',
      options: {},
    });
  } catch (caught: unknown) {
    recognitionError.value = t(caught instanceof Error ? caught.message : String(caught));
  } finally {
    recognitionBusy.value = false;
  }
}

function copyCode(data: string): void {
  if (postRequest({ type: 'clipboardWrite', text: data })) {
    message.success(t('ui.recognizedContentCopied'));
  } else {
    message.error(t('ui.clipboardIpcIsUnavailable'));
  }
}

async function generate(): Promise<void> {
  generationError.value = null;
  generated.value = false;
  await nextTick();
  if (canvas.value === null) {
    generationError.value = t('ui.theCanvasIsNotReady');
    return;
  }
  try {
    const options = buildBarcodeOptions({
      format: generationFormat.value,
      text: generationText.value,
      scale: generationScale.value ?? 4,
    });
    toCanvas(canvas.value, options);
    generated.value = true;
  } catch (caught: unknown) {
    generationError.value = t(caught instanceof Error ? caught.message : String(caught));
  }
}

function savePng(): void {
  if (canvas.value === null || !generated.value) return;
  canvas.value.toBlob((blob) => {
    if (blob === null) {
      generationError.value = t('ui.unableToExportPng');
      return;
    }
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `${generationFormat.value}.png`;
    link.click();
    URL.revokeObjectURL(url);
  }, 'image/png');
}

function releasePreview(): void {
  if (previewUrl.value !== null) URL.revokeObjectURL(previewUrl.value);
  previewUrl.value = null;
}
</script>

<template>
  <main class="barcode-studio">
    <header class="barcode-studio__header">
      <div>
        <h1>{{ t('ui.barcodeAndQrCode') }}</h1>
        <p>{{ t('ui.recognitionUsesLocalZbarGenerationRunsEntirelyInTheWebview') }}</p>
      </div>
      <NTag :bordered="false" size="small">
        {{ capability.version ?? t('ui.zbarIsNotInstalled') }}
      </NTag>
    </header>

    <NTabs v-model:value="activeTab" animated type="line">
      <NTabPane name="recognize" :tab="t('ui.recognize2')">
        <section class="barcode-studio__section">
          <NAlert v-if="!capability.available" type="warning">
            {{ t('ui.barcodeRecognitionIsMissingOnDebian13InstallZbarTools') }}
          </NAlert>
          <NAlert
            v-if="recognitionError !== null"
            closable
            type="error"
            @close="recognitionError = null"
          >
            {{ recognitionError }}
          </NAlert>
          <input
            ref="fileInput"
            class="barcode-studio__file-input"
            type="file"
            :accept="SUPPORTED_IMAGE_TYPES.join(',')"
            @change="handleFileInput"
          />
          <div class="barcode-studio__actions">
            <NButton @click="openFilePicker">{{ t('ui.chooseImage') }}</NButton>
            <NButton
              :disabled="selectedImage === null || !capability.available"
              :loading="recognitionBusy"
              type="primary"
              @click="recognize"
            >
              {{ t('ui.recognizeCode') }}
            </NButton>
          </div>
          <section class="barcode-studio__recognition-grid">
            <NCard class="barcode-studio__panel" :title="t('ui.imagePreview')" :bordered="false">
              <NSpin :show="recognitionBusy">
                <div
                  class="barcode-studio__drop-zone"
                  @click="openFilePicker"
                  @dragover.prevent
                  @drop.prevent="handleDrop"
                >
                  <img
                    v-if="previewUrl !== null"
                    :src="previewUrl"
                    :alt="t('ui.imageToRecognize')"
                  />
                  <NEmpty v-else :description="t('ui.dropAnImageOrPressCtrlVToPaste')" />
                </div>
              </NSpin>
            </NCard>
            <NCard
              class="barcode-studio__panel"
              :title="t('ui.recognitionResult')"
              :bordered="false"
            >
              <div
                v-if="recognitionResult !== null && recognitionResult.codes.length > 0"
                class="barcode-studio__codes"
              >
                <article
                  v-for="(code, index) in recognitionResult.codes"
                  :key="`${String(index)}-${code.codeType}-${code.data}`"
                  class="barcode-studio__code"
                >
                  <div class="barcode-studio__code-header">
                    <NTag size="small">{{ code.codeType }}</NTag>
                    <NButton size="small" @click="copyCode(code.data)">{{ t('ui.copy') }}</NButton>
                  </div>
                  <NCode :code="code.data" word-wrap />
                </article>
              </div>
              <NEmpty
                v-else
                :description="
                  recognitionResult === null
                    ? t('ui.waitingForRecognition')
                    : t('ui.noBarcodeOrQrCodeFound')
                "
              />
            </NCard>
          </section>
        </section>
      </NTabPane>

      <NTabPane name="generate" :tab="t('ui.generate')">
        <section class="barcode-studio__section">
          <NAlert
            v-if="generationError !== null"
            closable
            type="error"
            @close="generationError = null"
          >
            {{ generationError }}
          </NAlert>
          <NCard class="barcode-studio__panel" :bordered="false">
            <div class="barcode-studio__generator-form">
              <NSelect v-model:value="generationFormat" :options="formatOptions" />
              <CodeEditor
                v-model="generationText"
                max-height="14rem"
                min-height="6rem"
                :placeholder="t('ui.enterTextOrDigitsToEncode')"
              />
              <NInputNumber v-model:value="generationScale" :max="8" :min="1" />
              <div class="barcode-studio__actions">
                <NButton type="primary" @click="generate">{{ t('ui.generate') }}</NButton>
                <NButton :disabled="!generated" @click="savePng">{{ t('ui.savePng') }}</NButton>
              </div>
            </div>
          </NCard>
          <NCard class="barcode-studio__panel" :title="t('ui.generatedPreview')" :bordered="false">
            <div class="barcode-studio__canvas-wrap">
              <canvas ref="canvas" />
              <NEmpty v-if="!generated" :description="t('ui.setTheContentAndClickGenerate')" />
            </div>
          </NCard>
        </section>
      </NTabPane>
    </NTabs>
  </main>
</template>

<style scoped lang="scss">
.barcode-studio {
  display: grid;
  gap: var(--page-gap);
  height: var(--app-viewport-height);
  min-height: 0;
  overflow: auto;
  padding: var(--page-padding);

  &__header,
  &__actions,
  &__code-header {
    align-items: center;
    display: flex;
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

  &__section {
    display: grid;
    gap: 1rem;
  }

  &__actions,
  &__code-header {
    flex-wrap: wrap;
  }

  &__file-input {
    display: none;
  }

  &__recognition-grid {
    display: grid;
    gap: 1rem;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    min-height: 0;
  }

  &__panel {
    background: var(--panel-color);
  }

  &__drop-zone {
    cursor: pointer;
    display: grid;
    min-height: clamp(18rem, 54vh, 30rem);
    place-items: center;

    img {
      display: block;
      max-height: 62vh;
      max-width: 100%;
    }
  }

  &__codes {
    display: grid;
    gap: 0.75rem;
  }

  &__code {
    border: 1px solid var(--border-color);
    border-radius: 0.5rem;
    display: grid;
    gap: 0.75rem;
    padding: 0.75rem;
  }

  &__code-header {
    justify-content: space-between;
  }

  &__generator-form {
    display: grid;
    gap: 1rem;

    :deep(.n-select),
    :deep(.n-input-number) {
      max-width: 22rem;
    }
  }

  &__canvas-wrap {
    display: grid;
    min-height: clamp(15rem, 45vh, 24rem);
    overflow: auto;
    place-items: center;

    canvas {
      max-width: 100%;
    }
  }
}

@media (width <= 900px) {
  .barcode-studio {
    &__header {
      align-items: flex-start;
      flex-direction: column;
    }

    &__recognition-grid {
      grid-template-columns: 1fr;
    }
  }
}
</style>
