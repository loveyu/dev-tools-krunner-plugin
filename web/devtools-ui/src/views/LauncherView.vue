<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { NEmpty, NInput, NList, NListItem, NTag } from 'naive-ui';

import { useI18n } from '../i18n/runtime';
import { matchLauncherQuery, type LauncherAction } from '../tools/launcher/model';

defineOptions({ name: 'LauncherView' });

const props = defineProps<{ readonly activation: number }>();
const emit = defineEmits<{
  activate: [action: LauncherAction];
  close: [];
}>();
const { t } = useI18n();
const query = ref('');
const selected = ref(0);
const input = ref<InstanceType<typeof NInput> | null>(null);
const matches = computed(() => matchLauncherQuery(query.value));

watch(
  () => props.activation,
  () => {
    query.value = '';
    selected.value = 0;
    void nextTick(() => input.value?.focus());
  },
);
watch(query, () => {
  selected.value = 0;
});

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    emit('close');
    return;
  }
  if (event.key === 'ArrowDown') {
    event.preventDefault();
    selected.value = Math.min(selected.value + 1, matches.value.length - 1);
    return;
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault();
    selected.value = Math.max(selected.value - 1, 0);
    return;
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    const match = matches.value[selected.value];
    if (match !== undefined) emit('activate', match.action);
  }
}

onMounted(() => input.value?.focus());
</script>

<template>
  <main class="launcher-view" @keydown="handleKeydown">
    <NInput
      ref="input"
      v-model:value="query"
      autofocus
      clearable
      size="large"
      :placeholder="t('ui.enterACommandSearchToolsOrPasteJsonDirectly')"
    />

    <NList v-if="matches.length > 0" class="launcher-view__results" hoverable clickable>
      <NListItem
        v-for="(match, index) in matches"
        :key="match.id"
        :class="{ 'launcher-view__result--selected': selected === index }"
        @click="emit('activate', match.action)"
        @mouseenter="selected = index"
      >
        <div class="launcher-view__result">
          <div>
            <strong>{{ t(match.title) }}</strong>
            <p>{{ t(match.description) }}</p>
          </div>
          <NTag :bordered="false" size="small">{{ match.keywords[0] }}</NTag>
        </div>
      </NListItem>
    </NList>
    <NEmpty v-else :description="t('ui.noMatchingTools')" />

    <footer>
      {{ t('ui.selectEnterOpenEscCloseDirectJsonInputIsDetected') }}
    </footer>
  </main>
</template>

<style scoped lang="scss">
.launcher-view {
  display: grid;
  gap: 0.75rem;
  margin: 0 auto;
  max-width: 48rem;
  padding: 1.25rem;

  &__results {
    max-height: 22rem;
    overflow: auto;
  }

  &__result {
    align-items: center;
    display: flex;
    gap: 1rem;
    justify-content: space-between;
    width: 100%;

    p {
      color: var(--muted-color);
      margin: 0.2rem 0 0;
    }

    &--selected {
      background: color-mix(in srgb, var(--key-color) 13%, transparent);
    }
  }

  footer {
    color: var(--muted-color);
    font-size: 0.8rem;
    text-align: center;
  }
}
</style>
