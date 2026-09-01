<script setup lang="ts">
import { computed, inject, nextTick, onMounted, ref } from 'vue';
import {
  NButton,
  NColorPicker,
  NInput,
  NModal,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSlider,
  NSwitch,
  NTabPane,
  NTabs,
  useMessage,
} from 'naive-ui';

import { useI18n } from '../i18n/runtime';
import { OUTPUT_IMAGE_TYPES } from '../tools/image-compression/types';
import type { OutputImageType } from '../tools/image-compression/types';
import {
  DEFAULT_TIME_TEMPLATE,
  WATERMARK_LIMITS,
  WATERMARK_SETTINGS_KEY,
  createDefaultWatermarkSettings,
} from '../tools/watermark/model';
import type { WatermarkMode } from '../tools/watermark/model';
import { probeOutputSupport } from '../tools/watermark/renderer';

defineOptions({ name: 'WatermarkSettingsPanel' });

// 设置对象由父视图 provide；子组件直接改其字段以驱动实时预览。
const settings = inject(WATERMARK_SETTINGS_KEY, createDefaultWatermarkSettings(''));

defineProps<{
  watermarkFileName: string | null;
  canSave: boolean;
  saving: boolean;
  copyEnabled: boolean;
}>();

const emit = defineEmits<{
  chooseSource: [];
  chooseWatermark: [];
  save: [];
  copy: [];
}>();

const message = useMessage();
const { t } = useI18n();
const limits = WATERMARK_LIMITS;

const supportedTypes = ref<readonly OutputImageType[]>([...OUTPUT_IMAGE_TYPES]);
const formatOptions = computed(() =>
  supportedTypes.value.map((value) => ({
    label: value === 'image/jpeg' ? 'JPEG' : value === 'image/webp' ? 'WebP' : 'PNG',
    value,
  })),
);

const textModalVisible = ref(false);
const textDraft = ref('');
const timeTemplate = ref(DEFAULT_TIME_TEMPLATE);
const modalBody = ref<HTMLElement | null>(null);

onMounted(async () => {
  // WebKitGTK 可能缺少 WebP 编码；探测后不可选的格式直接隐藏。
  const supported = await probeOutputSupport([...OUTPUT_IMAGE_TYPES]);
  supportedTypes.value = supported;
  if (supported.length > 0 && !supported.includes(settings.outputType)) {
    const fallback = supported.find((type) => type === 'image/png') ?? supported[0];
    if (fallback !== undefined) {
      settings.outputType = fallback;
      message.warning(t('watermark.messages.webpUnavailable'));
    }
  }
});

function switchMode(mode: WatermarkMode): void {
  settings.mode = mode;
  if (mode === 'image') {
    emit('chooseWatermark');
  }
}

function openTextModal(): void {
  textDraft.value = settings.text;
  timeTemplate.value = DEFAULT_TIME_TEMPLATE;
  textModalVisible.value = true;
}

/** 在光标处插入时间格式串；格式输入框可自由修改占位符组合。 */
function insertTimeTemplate(): void {
  const template = timeTemplate.value.trim() || DEFAULT_TIME_TEMPLATE;
  const textarea = modalBody.value?.querySelector('textarea') ?? null;
  if (textarea === null) {
    textDraft.value += template;
    return;
  }
  const start = textarea.selectionStart;
  const end = textarea.selectionEnd;
  textDraft.value = textDraft.value.slice(0, start) + template + textDraft.value.slice(end);
  void nextTick(() => {
    textarea.focus();
    const caret = start + template.length;
    textarea.setSelectionRange(caret, caret);
  });
}

function confirmTextModal(): void {
  settings.text = textDraft.value;
  textModalVisible.value = false;
}
</script>

<template>
  <div class="watermark-panel">
    <NButton block type="primary" @click="emit('chooseSource')">
      {{ t('watermark.actions.chooseSource') }}
    </NButton>
    <NTabs type="line" animated>
      <NTabPane name="type" :tab="t('watermark.tabs.type')">
        <div class="watermark-panel__form">
          <NRadioGroup :value="settings.mode" name="watermark-mode" @update:value="switchMode">
            <NRadioButton value="text">{{ t('watermark.mode.text') }}</NRadioButton>
            <NRadioButton value="image">{{ t('watermark.mode.image') }}</NRadioButton>
          </NRadioGroup>
          <template v-if="settings.mode === 'text'">
            <NButton block @click="openTextModal">{{ t('watermark.actions.editText') }}</NButton>
            <p class="watermark-panel__hint">{{ settings.text }}</p>
          </template>
          <template v-else>
            <NButton block @click="emit('chooseWatermark')">
              {{ t('watermark.actions.chooseWatermarkImage') }}
            </NButton>
            <p v-if="watermarkFileName !== null" class="watermark-panel__hint">
              {{ watermarkFileName }}
            </p>
          </template>
        </div>
      </NTabPane>

      <NTabPane name="style" :tab="t('watermark.tabs.style')">
        <div class="watermark-panel__form">
          <label class="watermark-panel__field">
            <span
              >{{ t('watermark.style.opacity') }} · {{ Math.round(settings.opacity * 100) }}%</span
            >
            <NSlider
              v-model:value="settings.opacity"
              :max="limits.opacity.max"
              :min="limits.opacity.min"
              :step="limits.opacity.step"
              :format-tooltip="(value: number) => `${String(Math.round(value * 100))}%`"
            />
          </label>
          <label class="watermark-panel__field">
            <span>{{ t('watermark.style.angle') }} · {{ settings.angle }}°</span>
            <NSlider
              v-model:value="settings.angle"
              :max="limits.angle.max"
              :min="limits.angle.min"
              :step="limits.angle.step"
            />
          </label>
          <label class="watermark-panel__field">
            <span>{{ t('watermark.style.scale') }} · {{ settings.scale.toFixed(2) }}×</span>
            <NSlider
              v-model:value="settings.scale"
              :max="limits.scale.max"
              :min="limits.scale.min"
              :step="limits.scale.step"
            />
          </label>
          <template v-if="settings.mode === 'text'">
            <label class="watermark-panel__field">
              <span
                >{{ t('watermark.style.fontSize') }} · {{ settings.textStyle.fontSize }} px</span
              >
              <NSlider
                v-model:value="settings.textStyle.fontSize"
                :max="limits.fontSize.max"
                :min="limits.fontSize.min"
                :step="limits.fontSize.step"
              />
            </label>
            <label class="watermark-panel__field">
              <span
                >{{ t('watermark.style.fontWeight') }} · {{ settings.textStyle.fontWeight }}</span
              >
              <NSlider
                v-model:value="settings.textStyle.fontWeight"
                :max="limits.fontWeight.max"
                :min="limits.fontWeight.min"
                :step="limits.fontWeight.step"
              />
            </label>
            <div class="watermark-panel__switches">
              <span class="watermark-panel__switch">
                {{ t('watermark.style.textCenter') }}
                <NSwitch v-model:value="settings.textStyle.center" size="small" />
              </span>
              <span class="watermark-panel__switch">
                {{ t('watermark.style.outline') }}
                <NSwitch v-model:value="settings.textStyle.outline" size="small" />
              </span>
              <span class="watermark-panel__switch">
                {{ t('watermark.style.italic') }}
                <NSwitch v-model:value="settings.textStyle.italic" size="small" />
              </span>
              <span class="watermark-panel__switch">
                {{ t('watermark.style.shadow') }}
                <NSwitch v-model:value="settings.textStyle.shadow" size="small" />
              </span>
            </div>
            <div class="watermark-panel__split-fields">
              <label class="watermark-panel__field">
                <span>{{ t('watermark.style.textColor') }}</span>
                <NColorPicker v-model:value="settings.textStyle.textColor" :show-alpha="false" />
              </label>
              <label class="watermark-panel__field">
                <span>{{ t('watermark.style.shadowColor') }}</span>
                <NColorPicker v-model:value="settings.textStyle.shadowColor" :show-alpha="false" />
              </label>
            </div>
          </template>
        </div>
      </NTabPane>

      <NTabPane name="layout" :tab="t('watermark.tabs.layout')">
        <div class="watermark-panel__form">
          <label class="watermark-panel__field">
            <span>{{ t('watermark.layout.offsetX') }} · {{ settings.offsetX }}</span>
            <NSlider
              v-model:value="settings.offsetX"
              :max="limits.offset.max"
              :min="limits.offset.min"
              :step="limits.offset.step"
            />
          </label>
          <label class="watermark-panel__field">
            <span>{{ t('watermark.layout.offsetY') }} · {{ settings.offsetY }}</span>
            <NSlider
              v-model:value="settings.offsetY"
              :max="limits.offset.max"
              :min="limits.offset.min"
              :step="limits.offset.step"
            />
          </label>
          <label class="watermark-panel__field">
            <span>{{ t('watermark.layout.gapX') }} · {{ settings.gapX }}</span>
            <NSlider
              v-model:value="settings.gapX"
              :max="limits.gapX.max"
              :min="limits.gapX.min"
              :step="limits.gapX.step"
            />
          </label>
          <label class="watermark-panel__field">
            <span>{{ t('watermark.layout.gapY') }} · {{ settings.gapY }}</span>
            <NSlider
              v-model:value="settings.gapY"
              :max="limits.gapY.max"
              :min="limits.gapY.min"
              :step="limits.gapY.step"
            />
          </label>
          <template v-if="settings.mode === 'image'">
            <span class="watermark-panel__switch watermark-panel__switch--wide">
              {{ t('watermark.layout.keepImageVisible') }}
              <NSwitch v-model:value="settings.keepImageVisible" size="small" />
            </span>
            <p class="watermark-panel__hint">{{ t('watermark.layout.keepImageVisibleHint') }}</p>
          </template>
        </div>
      </NTabPane>

      <NTabPane name="saving" :tab="t('watermark.tabs.saving')">
        <div class="watermark-panel__form">
          <label class="watermark-panel__field">
            <span>{{ t('ui.outputFormat') }}</span>
            <NSelect v-model:value="settings.outputType" :options="formatOptions" />
          </label>
          <label class="watermark-panel__field">
            <span
              >{{ t('watermark.saving.quality') }} · {{ Math.round(settings.quality * 100) }}%</span
            >
            <NSlider
              v-model:value="settings.quality"
              :disabled="settings.outputType === 'image/png'"
              :max="limits.quality.max"
              :min="limits.quality.min"
              :step="limits.quality.step"
            />
          </label>
          <p class="watermark-panel__hint">{{ t('watermark.saving.realtimeHint') }}</p>
          <NButton
            block
            :disabled="!canSave"
            :loading="saving"
            type="primary"
            @click="emit('save')"
          >
            {{ t('watermark.actions.saveImage') }}
          </NButton>
          <NButton block :disabled="!copyEnabled" @click="emit('copy')">
            {{ t('ui.copyPng') }}
          </NButton>
        </div>
      </NTabPane>
    </NTabs>

    <NModal
      v-model:show="textModalVisible"
      :title="t('watermark.editor.title')"
      preset="card"
      class="watermark-panel__modal"
    >
      <div ref="modalBody" class="watermark-panel__modal-body">
        <NInput
          v-model:value="textDraft"
          type="textarea"
          :autosize="{ minRows: 5, maxRows: 12 }"
          :placeholder="t('watermark.editor.title')"
        />
        <div class="watermark-panel__modal-tools">
          <NInput v-model:value="timeTemplate" size="small" :placeholder="DEFAULT_TIME_TEMPLATE" />
          <NButton size="small" @click="insertTimeTemplate">
            {{ t('watermark.actions.insertTime') }}
          </NButton>
          <NButton size="small" @click="textDraft = ''">
            {{ t('watermark.actions.clearText') }}
          </NButton>
        </div>
        <p class="watermark-panel__hint">{{ t('watermark.editor.templateHint') }}</p>
        <div class="watermark-panel__modal-actions">
          <NButton size="small" type="primary" @click="confirmTextModal">
            {{ t('watermark.actions.confirm') }}
          </NButton>
        </div>
      </div>
    </NModal>
  </div>
</template>

<style scoped lang="scss">
.watermark-panel {
  display: grid;
  gap: 0.75rem;

  &__form,
  &__field {
    display: grid;
    gap: 0.5rem;
  }

  &__field,
  &__hint {
    color: var(--muted-color);
    font-size: 0.82rem;

    span {
      color: inherit;
    }
  }

  &__hint {
    margin: 0;
    white-space: pre-line;
    word-break: break-all;
  }

  &__switches {
    display: grid;
    gap: 0.4rem;
    grid-template-columns: 1fr 1fr;
  }

  &__switch {
    align-items: center;
    color: var(--muted-color);
    display: flex;
    font-size: 0.82rem;
    gap: 0.5rem;
    justify-content: space-between;
  }

  &__switch--wide {
    margin-top: 0.25rem;
  }

  &__split-fields {
    display: grid;
    gap: 0.75rem;
    grid-template-columns: 1fr 1fr;
  }

  &__modal {
    width: min(30rem, 92vw);
  }

  &__modal-body {
    display: grid;
    gap: 0.75rem;
  }

  &__modal-tools {
    display: grid;
    gap: 0.5rem;
    grid-template-columns: minmax(0, 1fr) auto auto;
  }

  &__modal-actions {
    display: flex;
    justify-content: flex-end;
  }
}

@media (width <= 520px) {
  .watermark-panel {
    &__split-fields,
    &__switches {
      grid-template-columns: 1fr;
    }
  }
}
</style>
