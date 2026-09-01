<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { NAlert, NButton, NCard, NEmpty, NInput, NSpace, NSpin, NTag, useMessage } from 'naive-ui';

import { useI18n } from '../i18n/runtime';
import { postRequest } from '../ipc/bridge';
import {
  MetadataCancelledError,
  pickAndReadMetadata,
  readImageMetadata,
} from '../ipc/native-metadata';
import { firstImageFile, prepareImage } from '../tools/media/image';
import type { MetadataCapabilities, MetadataDocument } from '../tools/metadata/types';

defineOptions({ name: 'MetadataView' });

defineProps<{ readonly capabilities: MetadataCapabilities }>();
const { t } = useI18n();
const message = useMessage();
const document = ref<MetadataDocument | null>(null);
const search = ref('');
const loading = ref(false);
const error = ref<string | null>(null);
const groups = computed(() => {
  const query = search.value.trim().toLocaleLowerCase();
  const grouped = new Map<string, { readonly name: string; readonly value: string }[]>();
  for (const field of document.value?.fields ?? []) {
    if (
      query !== '' &&
      !`${field.group}\n${field.name}\n${field.value}`.toLocaleLowerCase().includes(query)
    ) {
      continue;
    }
    const values = grouped.get(field.group) ?? [];
    values.push({ name: field.name, value: field.value });
    grouped.set(field.group, values);
  }
  return Array.from(grouped, ([name, fields]) => ({ name, fields }));
});

onMounted(() => {
  window.addEventListener('paste', handlePaste);
});
onBeforeUnmount(() => {
  window.removeEventListener('paste', handlePaste);
});

async function chooseFile(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    document.value = await pickAndReadMetadata();
  } catch (caught) {
    // 取消选择文件不是错误，静默返回。
    if (caught instanceof MetadataCancelledError) return;
    error.value = t(caught instanceof Error ? caught.message : String(caught));
  } finally {
    loading.value = false;
  }
}

function handlePaste(event: ClipboardEvent): void {
  if (event.clipboardData === null) return;
  const image = firstImageFile(event.clipboardData.files);
  if (image === null) return;
  event.preventDefault();
  void readPastedImage(image);
}

async function readPastedImage(image: File): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    document.value = await readImageMetadata(await prepareImage(image));
  } catch (caught) {
    error.value = t(caught instanceof Error ? caught.message : String(caught));
  } finally {
    loading.value = false;
  }
}

function copy(value: string): void {
  if (postRequest({ type: 'clipboardWrite', text: value })) {
    message.success(t('metadata.messages.copied'));
  } else {
    message.error(t('ui.clipboardIpcIsUnavailable'));
  }
}

function copyJson(): void {
  if (document.value !== null) copy(JSON.stringify(document.value, null, 2));
}
</script>

<template>
  <main class="metadata-view">
    <header>
      <div>
        <h1>{{ t('metadata.title') }}</h1>
        <p>{{ t('metadata.description') }}</p>
      </div>
      <NSpace align="center">
        <NTag :bordered="false">{{ t('metadata.actions.pasteImageHint') }}</NTag>
        <NButton type="primary" :loading="loading" @click="chooseFile">
          {{ t('metadata.actions.chooseFile') }}
        </NButton>
      </NSpace>
    </header>

    <NAlert type="info" :show-icon="false">
      {{ t('metadata.capabilities.builtin') }}: {{ capabilities.builtinVersion }} ·
      {{ t('metadata.capabilities.external') }}:
      {{ capabilities.externalVersion ?? t('metadata.capabilities.notInstalled') }}
    </NAlert>
    <NAlert v-if="error !== null" type="error">{{ error }}</NAlert>

    <NSpin :show="loading">
      <template v-if="document !== null">
        <div class="metadata-view__toolbar">
          <NSpace align="center">
            <strong>{{ document.fileName }}</strong>
            <NTag :bordered="false">{{ document.backend }}</NTag>
            <NTag :bordered="false">{{ document.fields.length }}</NTag>
          </NSpace>
          <NSpace>
            <NInput v-model:value="search" clearable :placeholder="t('metadata.fields.search')" />
            <NButton @click="copyJson">{{ t('metadata.actions.copyJson') }}</NButton>
          </NSpace>
        </div>

        <div v-if="groups.length > 0" class="metadata-view__groups">
          <NCard v-for="group in groups" :key="group.name" :title="group.name" size="small">
            <dl>
              <template v-for="field in group.fields" :key="`${group.name}:${field.name}`">
                <dt>{{ field.name }}</dt>
                <dd>
                  <code>{{ field.value }}</code>
                  <NButton text size="tiny" @click="copy(field.value)">{{ t('ui.copy') }}</NButton>
                </dd>
              </template>
            </dl>
          </NCard>
        </div>
        <NEmpty v-else :description="t('metadata.empty.noMatches')" />
      </template>
      <NEmpty v-else :description="t('metadata.empty.chooseMedia')" />
    </NSpin>

    <footer class="open-source-attribution">
      <span>{{ t('metadata.opensource.uses') }}</span>
      <a href="https://exiftool.org/" rel="noreferrer" target="_blank">ExifTool</a>
      <span>·</span>
      <a href="https://github.com/vbasky/revelo" rel="noreferrer" target="_blank">revelo</a>
    </footer>
  </main>
</template>

<style scoped lang="scss">
.metadata-view {
  display: grid;
  gap: var(--page-gap);
  height: var(--app-viewport-height);
  min-height: 0;
  overflow: auto;
  padding: var(--page-padding);
  padding-inline: max(var(--page-padding), calc((100% - 74rem) / 2));

  header,
  &__toolbar {
    align-items: center;
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
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

  &__groups {
    display: grid;
    gap: 0.75rem;
    margin-top: 1rem;
  }

  &__toolbar {
    :deep(.n-space:first-child) {
      min-width: 0;

      strong {
        overflow-wrap: anywhere;
      }
    }

    :deep(.n-space:last-child) {
      flex: 1 1 20rem;
      justify-content: flex-end;

      .n-input {
        flex: 1 1 14rem;
      }
    }
  }

  dl {
    display: grid;
    gap: 0;
    grid-template-columns: minmax(10rem, 0.32fr) minmax(0, 1fr);
    margin: 0;
  }

  dt,
  dd {
    border-bottom: 1px solid var(--border-color);
    margin: 0;
    padding: 0.55rem;
  }

  dd {
    align-items: flex-start;
    display: flex;
    gap: 0.75rem;
    justify-content: space-between;
    overflow-wrap: anywhere;
  }
}

@media (width <= 720px) {
  .metadata-view {
    header,
    &__toolbar {
      align-items: stretch;
      flex-direction: column;
    }

    &__toolbar :deep(.n-space:last-child) {
      justify-content: flex-start;
      width: 100%;
    }
  }
}

@media (width <= 600px) {
  .metadata-view {
    dl {
      grid-template-columns: 1fr;
    }

    dt {
      border-bottom: 0;
      color: var(--muted-color);
      font-size: 0.78rem;
      padding-bottom: 0.2rem;
    }

    dd {
      padding-top: 0.2rem;
    }
  }
}
</style>
