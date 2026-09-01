<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NAlert, NButton, NCard, NSelect, NSpin, NTag, useMessage } from 'naive-ui';

import CodeEditor from '../components/CodeEditor.vue';
import { postRequest } from '../ipc/bridge';
import { useI18n } from '../i18n/runtime';
import { executeNativeConversion } from '../ipc/native-converter';
import { languageOfFormat } from '../tools/editor/languages';
import { convertText } from '../tools/converter/core';
import { detectFormat } from '../tools/converter/detect';
import { definitionOf, FORMAT_DEFINITIONS } from '../tools/converter/formats';
import type {
  ConverterCapabilities,
  FormatDefinition,
  FormatId,
  NativeFormatId,
} from '../tools/converter/types';

defineOptions({ name: 'DataConvertView' });

const props = defineProps<{
  readonly activation: number;
  readonly canGoBack: boolean;
  readonly capabilities: ConverterCapabilities;
  readonly payload: string;
  readonly sourceHint: FormatId | null;
}>();

const emit = defineEmits<{
  back: [];
}>();

const message = useMessage();
const { t } = useI18n();
const sourceText = ref<string>('');
const outputText = ref<string>('');
const sourceFormat = ref<FormatId>('plain');
const targetFormat = ref<FormatId>('json');
const busy = ref<boolean>(false);
const error = ref<string | null>(null);

const sourceOptions = computed(() =>
  FORMAT_DEFINITIONS.filter((definition) => definition.canParse).map(toSelectOption),
);
const targetOptions = computed(() =>
  FORMAT_DEFINITIONS.filter((definition) => definition.canStringify).map(toSelectOption),
);
const availableSources = computed(
  () =>
    new Set(
      FORMAT_DEFINITIONS.filter(
        (definition) => definition.canParse && isDefinitionAvailable(definition),
      ).map((definition) => definition.id),
    ),
);
const sourceLanguage = computed(() => languageOfFormat(sourceFormat.value));
const targetLanguage = computed(() => languageOfFormat(targetFormat.value));
const phpLabel = computed(() =>
  props.capabilities.phpVersion === null
    ? t('ui.phpCliIsNotInstalled')
    : `PHP ${props.capabilities.phpVersion}`,
);

watch(
  () => props.activation,
  () => {
    sourceText.value = props.payload;
    sourceFormat.value =
      props.sourceHint !== null && availableSources.value.has(props.sourceHint)
        ? props.sourceHint
        : detectFormat(props.payload, availableSources.value);
    targetFormat.value = 'json';
    outputText.value = '';
    error.value = null;
    void runConversion();
  },
  { immediate: true },
);

async function runConversion(): Promise<void> {
  busy.value = true;
  error.value = null;
  try {
    outputText.value = await convertText(
      sourceText.value,
      sourceFormat.value,
      targetFormat.value,
      executeNativeConversion,
    );
  } catch (caught: unknown) {
    error.value = t(caught instanceof Error ? caught.message : String(caught));
  } finally {
    busy.value = false;
  }
}

function detectSource(): void {
  sourceFormat.value = detectFormat(sourceText.value, availableSources.value);
  void runConversion();
}

function exchangeData(): void {
  const nextSource = definitionOf(targetFormat.value);
  const nextTarget = definitionOf(sourceFormat.value);
  if (
    outputText.value === '' ||
    !nextSource.canParse ||
    !nextTarget.canStringify ||
    !isDefinitionAvailable(nextSource) ||
    !isDefinitionAvailable(nextTarget)
  ) {
    message.warning(t('ui.thisFormatPairCannotBeSwapped'));
    return;
  }
  const previousSourceText = sourceText.value;
  sourceText.value = outputText.value;
  outputText.value = previousSourceText;
  const previousSourceFormat = sourceFormat.value;
  sourceFormat.value = targetFormat.value;
  targetFormat.value = previousSourceFormat;
  error.value = null;
}

function copyOutput(): void {
  if (postRequest({ type: 'clipboardWrite', text: outputText.value })) {
    message.success(t('ui.conversionResultCopied'));
  } else {
    message.error(t('ui.clipboardIpcIsUnavailable'));
  }
}

function isDefinitionAvailable(definition: FormatDefinition): boolean {
  return (
    definition.runtime === 'web' ||
    props.capabilities.nativeFormats.includes(definition.id as NativeFormatId)
  );
}

function toSelectOption(definition: FormatDefinition): {
  readonly label: string;
  readonly value: FormatId;
  readonly disabled: boolean;
} {
  const available = isDefinitionAvailable(definition);
  return {
    label: available ? t(definition.label) : `${t(definition.label)}（${t('ui.requiresPhpCli')}）`,
    value: definition.id,
    disabled: !available,
  };
}
</script>

<template>
  <main class="converter">
    <header class="converter__header">
      <div class="converter__title">
        <NButton v-if="canGoBack" quaternary @click="emit('back')">{{
          t('ui.backToJson')
        }}</NButton>
        <div>
          <h1>{{ t('ui.dataConversion') }}</h1>
          <p>{{ t('ui.conversionsRunLocallyInTheWebviewWhenPossibleSystemCapabilities') }}</p>
        </div>
      </div>
      <!-- 主操作放头部：页面底部长时间无人注视，放那里容易被忽略。 -->
      <div class="converter__header-actions">
        <NButton @click="exchangeData">{{ t('ui.swap') }}</NButton>
        <NButton :loading="busy" type="primary" @click="runConversion">
          {{ t('ui.convert') }}
        </NButton>
        <NTag :bordered="false" size="small">{{ phpLabel }}</NTag>
      </div>
    </header>

    <NAlert v-if="error !== null" closable type="error" @close="error = null">
      {{ error }}
    </NAlert>

    <section class="converter__panels">
      <NCard class="converter__panel" :bordered="false">
        <template #header>
          <div class="converter__panel-toolbar">
            <strong>{{ t('ui.source') }}</strong>
            <NSelect v-model:value="sourceFormat" filterable :options="sourceOptions" />
            <NButton @click="detectSource">{{ t('ui.detect') }}</NButton>
          </div>
        </template>
        <CodeEditor
          v-model="sourceText"
          :language="sourceLanguage"
          :placeholder="t('ui.enterOrPasteTextToConvert')"
        />
      </NCard>

      <NCard class="converter__panel" :bordered="false">
        <template #header>
          <div class="converter__panel-toolbar">
            <strong>{{ t('ui.target') }}</strong>
            <NSelect v-model:value="targetFormat" filterable :options="targetOptions" />
            <NButton :disabled="outputText === ''" @click="copyOutput">{{ t('ui.copy') }}</NButton>
          </div>
        </template>
        <NSpin class="converter__spin" :show="busy">
          <CodeEditor
            v-model="outputText"
            :language="targetLanguage"
            :placeholder="t('ui.conversionResult')"
            readonly
          />
        </NSpin>
      </NCard>
    </section>
  </main>
</template>

<style scoped lang="scss">
.converter {
  display: flex;
  flex-direction: column;
  gap: var(--page-gap);
  height: var(--app-viewport-height);
  min-height: 0;
  overflow: auto;
  padding: var(--page-padding);

  &__header,
  &__title,
  &__header-actions,
  &__panel-toolbar {
    align-items: center;
    display: flex;
    flex-shrink: 0;
    gap: 0.75rem;
  }

  &__header {
    flex-wrap: wrap;
    justify-content: space-between;
  }

  &__header-actions {
    flex-wrap: wrap;
  }

  &__title {
    h1,
    p {
      margin: 0;
    }

    p {
      color: var(--muted-color);
      margin-top: 0.25rem;
    }
  }

  &__panels {
    display: grid;
    flex: 1 1 auto;
    gap: 1rem;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    grid-template-rows: minmax(14rem, 1fr);
    min-height: 14rem;
  }

  &__panel {
    background: var(--panel-color);
    display: flex;
    flex-direction: column;
    min-height: 14rem;
    min-width: 0;

    // 编辑器填满卡片剩余高度，由 CodeMirror 内部滚动。
    :deep(.n-card-content) {
      display: flex;
      flex: 1 1 auto;
      flex-direction: column;
      min-height: 0;
    }

    :deep(.code-editor) {
      flex: 1 1 auto;
    }
  }

  &__panel-toolbar {
    flex-wrap: wrap;

    :deep(.n-select) {
      flex: 1 1 11rem;
      min-width: 13rem;
    }
  }

  &__spin {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-height: 0;

    :deep(.n-spin-container),
    :deep(.n-spin-content) {
      display: flex;
      flex: 1 1 auto;
      flex-direction: column;
      min-height: 0;
    }

    :deep(.code-editor) {
      flex: 1 1 auto;
    }
  }
}

@media (width <= 920px) {
  .converter {
    &__header {
      align-items: flex-start;
      flex-direction: column;
    }

    &__panels {
      // 窄屏堆叠时退出行约束，每个面板保持可用高度并由页面滚动。
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: none;
    }
  }
}

@media (width <= 560px) {
  .converter {
    &__title,
    &__panel-toolbar {
      align-items: stretch;
      flex-direction: column;

      :deep(.n-select) {
        min-width: 0;
      }
    }
  }
}
</style>
