<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NButtonGroup, NCard, NEmpty, NInput, NTag, useMessage } from 'naive-ui';

import JsonTreeNode from '../components/JsonTreeNode.vue';
import { useI18n } from '../i18n/runtime';
import { postRequest } from '../ipc/bridge';
import {
  buildJsonTree,
  countMatches,
  filterJsonTree,
  formatJson,
  minifyJson,
  parseJson,
} from '../tools/json/model';

defineOptions({ name: 'JsonWorkbench' });

const props = defineProps<{
  readonly payload: string;
}>();

const emit = defineEmits<{
  convert: [payload: string];
}>();

type OutputMode = 'formatted' | 'minified';

const message = useMessage();
const { t } = useI18n();
const search = ref<string>('');
const outputMode = ref<OutputMode>('formatted');
const value = computed(() => parseJson(props.payload));
const tree = computed(() => buildJsonTree(value.value));
const filteredTree = computed(() => filterJsonTree(tree.value, search.value));
const matchCount = computed(() => countMatches(tree.value, search.value));
const output = computed(() =>
  outputMode.value === 'formatted' ? formatJson(value.value) : minifyJson(value.value),
);

watch(
  () => props.payload,
  () => {
    search.value = '';
    outputMode.value = 'formatted';
  },
);

function copyOutput(): void {
  if (postRequest({ type: 'clipboardWrite', text: output.value })) {
    message.success(t('ui.currentJsonCopied'));
  } else {
    message.error(t('ui.clipboardIpcIsUnavailable'));
  }
}
</script>

<template>
  <main class="workbench">
    <header class="workbench__toolbar">
      <div>
        <h1>JSON Workbench</h1>
        <p>{{ t('ui.parsedLocallyClipboardContentIsNeverUploaded') }}</p>
      </div>
      <NButtonGroup>
        <NButton
          :type="outputMode === 'formatted' ? 'primary' : 'default'"
          @click="outputMode = 'formatted'"
        >
          {{ t('ui.format') }}
        </NButton>
        <NButton
          :type="outputMode === 'minified' ? 'primary' : 'default'"
          @click="outputMode = 'minified'"
        >
          {{ t('ui.minify') }}
        </NButton>
        <NButton type="primary" secondary @click="copyOutput">{{ t('ui.copy') }}</NButton>
        <NButton secondary @click="emit('convert', output)">{{ t('ui.dataConversion') }}</NButton>
      </NButtonGroup>
    </header>

    <section class="workbench__search">
      <NInput v-model:value="search" clearable :placeholder="t('ui.searchKeysPathsOrValues')" />
      <NTag v-if="search.trim().length > 0" :bordered="false" size="small">
        {{ t('ui.countMatches', { count: matchCount }) }}
      </NTag>
    </section>

    <section class="workbench__content">
      <NCard class="workbench__panel" :title="t('ui.treeView')" :bordered="false">
        <ul v-if="filteredTree !== null" class="workbench__tree">
          <JsonTreeNode :node="filteredTree" />
        </ul>
        <NEmpty v-else :description="t('ui.noMatchingJsonNodes')" />
      </NCard>

      <NCard class="workbench__panel" :title="t('ui.textPreview')" :bordered="false">
        <pre class="workbench__output">{{ output }}</pre>
      </NCard>
    </section>
  </main>
</template>

<style scoped lang="scss">
.workbench {
  display: grid;
  gap: 1rem;
  min-height: 100%;
  padding: 1.25rem;

  &__toolbar,
  &__search {
    align-items: center;
    display: flex;
    gap: 1rem;
    justify-content: space-between;
  }

  &__toolbar {
    h1,
    p {
      margin: 0;
    }

    p {
      color: var(--muted-color);
      margin-top: 0.25rem;
    }
  }

  &__search :deep(.n-input) {
    max-width: 38rem;
  }

  &__content {
    display: grid;
    gap: 1rem;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    min-height: 0;
  }

  &__panel {
    background: var(--panel-color);
    min-height: 30rem;
  }

  &__tree {
    margin: 0;
    padding: 0;
  }

  &__output {
    font-family: var(--font-code);
    line-height: 1.6;
    margin: 0;
    overflow: auto;
    white-space: pre-wrap;
  }
}

@media (width <= 860px) {
  .workbench {
    &__toolbar {
      align-items: flex-start;
      flex-direction: column;
    }

    &__content {
      grid-template-columns: 1fr;
    }
  }
}
</style>
