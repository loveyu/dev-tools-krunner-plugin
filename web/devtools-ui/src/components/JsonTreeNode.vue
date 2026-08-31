<script setup lang="ts">
import { computed } from 'vue';

import type { JsonTreeNode } from '../tools/json/model';

defineOptions({ name: 'JsonTreeNode' });

const props = defineProps<{
  readonly node: JsonTreeNode;
}>();

const isBranch = computed<boolean>(() => props.node.type !== 'primitive');
</script>

<template>
  <li class="json-node">
    <details v-if="isBranch" class="json-node__branch" open>
      <summary>
        <span class="json-node__key">{{ node.key }}</span>
        <span class="json-node__preview">{{ node.preview }}</span>
      </summary>
      <ul class="json-node__children">
        <JsonTreeNode v-for="child in node.children" :key="child.id" :node="child" />
      </ul>
    </details>
    <div v-else class="json-node__leaf">
      <span class="json-node__key">{{ node.key }}</span>
      <span class="json-node__separator">:</span>
      <span class="json-node__value">{{ node.preview }}</span>
    </div>
  </li>
</template>

<style scoped lang="scss">
.json-node {
  font-family: var(--font-code);
  line-height: 1.8;
  list-style: none;

  &__branch > summary {
    cursor: pointer;
    user-select: none;
  }

  &__children {
    border-left: 1px solid var(--border-color);
    margin: 0 0 0 0.45rem;
    padding-left: 1rem;
  }

  &__key {
    color: var(--key-color);
    font-weight: 650;
  }

  &__preview,
  &__separator {
    color: var(--muted-color);
    margin-left: 0.45rem;
  }

  &__value {
    color: var(--value-color);
    margin-left: 0.45rem;
    overflow-wrap: anywhere;
  }
}
</style>
