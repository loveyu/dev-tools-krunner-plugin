<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NAlert, NButton, NCard, NInput, NSelect, NSpin, NTag, useMessage } from 'naive-ui';

import { postRequest } from '../ipc/bridge';
import { executeNativeConversion } from '../ipc/native-converter';
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
const phpLabel = computed(() =>
  props.capabilities.phpVersion === null
    ? 'PHP CLI 未安装'
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
    error.value = caught instanceof Error ? caught.message : String(caught);
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
    message.warning('当前格式组合不支持交换');
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
    message.success('已复制转换结果');
  } else {
    message.error('当前环境未提供剪贴板 IPC');
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
    label: available ? definition.label : `${definition.label}（需要 PHP CLI）`,
    value: definition.id,
    disabled: !available,
  };
}
</script>

<template>
  <main class="converter">
    <header class="converter__header">
      <div class="converter__title">
        <NButton v-if="canGoBack" quaternary @click="emit('back')">返回 JSON</NButton>
        <div>
          <h1>数据转换</h1>
          <p>优先在 WebView 本地转换；系统能力通过 Rust 安全调用</p>
        </div>
      </div>
      <NTag :bordered="false" size="small">{{ phpLabel }}</NTag>
    </header>

    <NAlert v-if="error !== null" closable type="error" @close="error = null">
      {{ error }}
    </NAlert>

    <section class="converter__panels">
      <NCard class="converter__panel" :bordered="false">
        <template #header>
          <div class="converter__panel-toolbar">
            <strong>来源</strong>
            <NSelect v-model:value="sourceFormat" filterable :options="sourceOptions" />
            <NButton @click="detectSource">探测</NButton>
          </div>
        </template>
        <NInput
          v-model:value="sourceText"
          :autosize="{ minRows: 20, maxRows: 32 }"
          class="converter__editor"
          placeholder="输入或粘贴待转换文本"
          type="textarea"
        />
      </NCard>

      <NCard class="converter__panel" :bordered="false">
        <template #header>
          <div class="converter__panel-toolbar">
            <strong>目标</strong>
            <NSelect v-model:value="targetFormat" filterable :options="targetOptions" />
            <NButton :disabled="outputText === ''" @click="copyOutput">复制</NButton>
          </div>
        </template>
        <NSpin :show="busy">
          <NInput
            v-model:value="outputText"
            :autosize="{ minRows: 20, maxRows: 32 }"
            class="converter__editor"
            placeholder="转换结果"
            readonly
            type="textarea"
          />
        </NSpin>
      </NCard>
    </section>

    <footer class="converter__actions">
      <NButton @click="exchangeData">交换</NButton>
      <NButton :loading="busy" type="primary" @click="runConversion">转换</NButton>
    </footer>
  </main>
</template>

<style scoped lang="scss">
.converter {
  display: grid;
  gap: 1rem;
  min-height: 100%;
  padding: 1.25rem;

  &__header,
  &__title,
  &__panel-toolbar,
  &__actions {
    align-items: center;
    display: flex;
    gap: 0.75rem;
  }

  &__header {
    justify-content: space-between;
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
    gap: 1rem;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }

  &__panel {
    background: var(--panel-color);
  }

  &__panel-toolbar {
    :deep(.n-select) {
      min-width: 13rem;
    }
  }

  &__editor {
    font-family: var(--font-code);
  }

  &__actions {
    justify-content: center;
  }
}

@media (width <= 860px) {
  .converter {
    &__header,
    &__title {
      align-items: flex-start;
      flex-direction: column;
    }

    &__panels {
      grid-template-columns: 1fr;
    }

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
