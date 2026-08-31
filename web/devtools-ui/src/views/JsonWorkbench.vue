<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NButtonGroup, NCard, NEmpty, NInput, NTag, useMessage } from 'naive-ui';

import JsonTreeNode from '../components/JsonTreeNode.vue';
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
    message.success('已复制当前 JSON');
  } else {
    message.error('当前环境未提供剪贴板 IPC');
  }
}
</script>

<template>
  <main class="workbench">
    <header class="workbench__toolbar">
      <div>
        <h1>JSON Workbench</h1>
        <p>本地解析，不上传剪贴板内容</p>
      </div>
      <NButtonGroup>
        <NButton
          :type="outputMode === 'formatted' ? 'primary' : 'default'"
          @click="outputMode = 'formatted'"
        >
          格式化
        </NButton>
        <NButton
          :type="outputMode === 'minified' ? 'primary' : 'default'"
          @click="outputMode = 'minified'"
        >
          压缩
        </NButton>
        <NButton type="primary" secondary @click="copyOutput">复制</NButton>
        <NButton secondary @click="emit('convert', output)">数据转换</NButton>
      </NButtonGroup>
    </header>

    <section class="workbench__search">
      <NInput v-model:value="search" clearable placeholder="搜索键、路径或值" />
      <NTag v-if="search.trim().length > 0" :bordered="false" size="small">
        {{ matchCount }} 个命中
      </NTag>
    </section>

    <section class="workbench__content">
      <NCard class="workbench__panel" title="树视图" :bordered="false">
        <ul v-if="filteredTree !== null" class="workbench__tree">
          <JsonTreeNode :node="filteredTree" />
        </ul>
        <NEmpty v-else description="没有匹配的 JSON 节点" />
      </NCard>

      <NCard class="workbench__panel" title="文本预览" :bordered="false">
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
