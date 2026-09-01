<script setup lang="ts">
import { computed } from 'vue';
import {
  NAlert,
  NButton,
  NCard,
  NInput,
  NInputNumber,
  NList,
  NListItem,
  NSelect,
  NSwitch,
  NTag,
} from 'naive-ui';

import { useI18n } from '../i18n/runtime';
import type { LanguageMode, MetadataBackend, Settings, ThemeMode } from '../ipc/types';
import type { MetadataCapabilities } from '../tools/metadata/types';

defineOptions({ name: 'SettingsView' });

const props = defineProps<{
  readonly settings: Settings;
  readonly version: string;
  readonly canGoBack: boolean;
  readonly error: string | null;
  readonly metadataCapabilities: MetadataCapabilities;
}>();

const emit = defineEmits<{
  back: [];
  update: [settings: Settings];
}>();
const { t } = useI18n();

const themeOptions = computed<{ readonly label: string; readonly value: ThemeMode }[]>(() => [
  { label: t('ui.followSystem'), value: 'system' },
  { label: t('ui.light'), value: 'light' },
  { label: t('ui.dark'), value: 'dark' },
]);
const languageOptions = computed<{ readonly label: string; readonly value: LanguageMode }[]>(() => [
  { label: t('ui.autoDetect'), value: 'system' },
  { label: t('ui.simplifiedChinese'), value: 'zh-CN' },
  { label: t('ui.traditionalChinese'), value: 'zh-TW' },
  { label: t('ui.english'), value: 'en-US' },
]);
const metadataBackendOptions = computed<
  { readonly label: string; readonly value: MetadataBackend; readonly disabled?: boolean }[]
>(() => [
  { label: t('settings.metadata.builtin'), value: 'builtin' },
  {
    label: t('settings.metadata.external'),
    value: 'external',
    disabled: !props.metadataCapabilities.externalAvailable,
  },
]);

function updateShowTray(showTray: boolean): void {
  emit('update', { ...props.settings, showTray });
}

function updateAutostart(autostart: boolean): void {
  emit('update', { ...props.settings, autostart });
}

function updateGlobalShortcutEnabled(globalShortcutEnabled: boolean): void {
  emit('update', { ...props.settings, globalShortcutEnabled });
}

function updateGlobalShortcut(globalShortcut: string): void {
  emit('update', { ...props.settings, globalShortcut: globalShortcut.trim() });
}

function updateQuickInputEnabled(quickInputEnabled: boolean): void {
  emit('update', { ...props.settings, quickInputEnabled });
}

function updateQuickInputShortcut(quickInputShortcut: string): void {
  emit('update', { ...props.settings, quickInputShortcut: quickInputShortcut.trim() });
}

function updateQuickInputWidth(quickInputWidth: number | null): void {
  if (quickInputWidth !== null) emit('update', { ...props.settings, quickInputWidth });
}

function updateQuickInputHeight(quickInputHeight: number | null): void {
  if (quickInputHeight !== null) emit('update', { ...props.settings, quickInputHeight });
}

function updateTheme(theme: ThemeMode): void {
  emit('update', { ...props.settings, theme });
}

function updateLanguage(language: LanguageMode): void {
  emit('update', { ...props.settings, language });
}

function updateMetadataBackend(metadataBackend: MetadataBackend): void {
  emit('update', { ...props.settings, metadataBackend });
}
</script>

<template>
  <main class="settings-view">
    <header class="settings-view__header">
      <div>
        <h1>{{ t('ui.devtoolsSettings') }}</h1>
        <p>{{ t('ui.workerAndDesktopIntegration') }}</p>
      </div>
      <NButton v-if="canGoBack" secondary @click="emit('back')">{{ t('ui.backToJson') }}</NButton>
    </header>

    <NAlert v-if="error !== null" :title="t('ui.failedToSaveSettings')" type="error">
      {{ error }}
    </NAlert>

    <NCard :title="t('ui.general')" :bordered="false">
      <NList>
        <NListItem>
          <div class="settings-view__item">
            <div>
              <strong>{{ t('settings.metadata.title') }}</strong>
              <p>
                {{ t('settings.metadata.description') }}
                {{ metadataCapabilities.externalVersion ?? t('settings.metadata.externalMissing') }}
              </p>
            </div>
            <NSelect
              class="settings-view__metadata-select"
              :options="metadataBackendOptions"
              :value="settings.metadataBackend"
              @update:value="updateMetadataBackend"
            />
          </div>
        </NListItem>
        <NListItem>
          <div class="settings-view__item settings-view__item--top">
            <div>
              <strong>{{ t('settings.quickInput.title') }}</strong>
              <p>
                {{ t('settings.quickInput.description') }}
              </p>
            </div>
            <div class="settings-view__quick-input">
              <div class="settings-view__shortcut">
                <NInput
                  :disabled="!settings.quickInputEnabled"
                  :value="settings.quickInputShortcut"
                  placeholder="Ctrl+Alt+KeyI"
                  @change="updateQuickInputShortcut"
                />
                <NSwitch
                  :value="settings.quickInputEnabled"
                  @update:value="updateQuickInputEnabled"
                />
              </div>
              <div class="settings-view__dimensions">
                <span>{{ t('common.width') }}</span>
                <NInputNumber
                  :disabled="!settings.quickInputEnabled"
                  :max="1600"
                  :min="240"
                  :step="20"
                  :value="settings.quickInputWidth"
                  @update:value="updateQuickInputWidth"
                />
                <span>{{ t('common.height') }}</span>
                <NInputNumber
                  :disabled="!settings.quickInputEnabled"
                  :max="240"
                  :min="40"
                  :step="4"
                  :value="settings.quickInputHeight"
                  @update:value="updateQuickInputHeight"
                />
              </div>
            </div>
          </div>
        </NListItem>
        <NListItem>
          <div class="settings-view__item">
            <div>
              <strong>{{ t('ui.globalShortcut') }}</strong>
              <p>{{ t('ui.openTheKrunnerLikeLauncherItIsOffByDefault') }}</p>
            </div>
            <div class="settings-view__shortcut">
              <NInput
                :disabled="!settings.globalShortcutEnabled"
                :value="settings.globalShortcut"
                placeholder="Ctrl+Alt+Space"
                @change="updateGlobalShortcut"
              />
              <NSwitch
                :value="settings.globalShortcutEnabled"
                @update:value="updateGlobalShortcutEnabled"
              />
            </div>
          </div>
        </NListItem>
        <NListItem>
          <div class="settings-view__item">
            <div>
              <strong>{{ t('ui.showTrayIcon') }}</strong>
              <p>{{ t('ui.showDevtoolsInSystemTray') }}</p>
            </div>
            <NSwitch :value="settings.showTray" @update:value="updateShowTray" />
          </div>
        </NListItem>
        <NListItem>
          <div class="settings-view__item">
            <div>
              <strong>{{ t('ui.appearance') }}</strong>
              <p>{{ t('ui.followSystemThemeOrChooseLightDark') }}</p>
            </div>
            <NSelect
              class="settings-view__theme-select"
              :options="themeOptions"
              :value="settings.theme"
              @update:value="updateTheme"
            />
          </div>
        </NListItem>
        <NListItem>
          <div class="settings-view__item">
            <div>
              <strong>{{ t('ui.language') }}</strong>
              <p>
                {{ t('ui.detectTheSystemLanguageByDefaultOrChooseSimplifiedChinese') }}
              </p>
            </div>
            <NSelect
              class="settings-view__theme-select"
              :options="languageOptions"
              :value="settings.language"
              @update:value="updateLanguage"
            />
          </div>
        </NListItem>
        <NListItem>
          <div class="settings-view__item">
            <div>
              <strong>{{ t('ui.startAutomatically') }}</strong>
              <p>{{ t('ui.startWorkerAfterSignIn') }}</p>
            </div>
            <NSwitch :value="settings.autostart" @update:value="updateAutostart" />
          </div>
        </NListItem>
      </NList>
    </NCard>

    <NAlert :title="t('ui.runtime')" type="info" :show-icon="false">
      <div class="settings-view__runtime">
        <span>{{ t('ui.version') }}</span>
        <NTag :bordered="false">{{ version }}</NTag>
        <span>{{ t('ui.platform') }}</span>
        <NTag :bordered="false">Linux Wayland / X11 · Windows</NTag>
      </div>
    </NAlert>
  </main>
</template>

<style scoped lang="scss">
.settings-view {
  display: grid;
  gap: 1rem;
  margin: 0 auto;
  max-width: 52rem;
  padding: 1.5rem;

  &__header,
  &__item,
  &__runtime {
    align-items: center;
    display: flex;
    gap: 1rem;
    justify-content: space-between;
  }

  &__header {
    h1,
    p {
      margin: 0;
    }

    p {
      color: var(--muted-color);
    }
  }

  &__item {
    width: 100%;

    p {
      color: var(--muted-color);
      margin: 0.35rem 0 0;
    }

    &--top {
      align-items: flex-start;
    }
  }

  &__runtime {
    justify-content: flex-start;
  }

  &__theme-select {
    flex: 0 0 9rem;
  }

  &__metadata-select {
    flex: 0 0 14rem;
  }

  &__shortcut {
    align-items: center;
    display: flex;
    flex: 0 0 18rem;
    gap: 0.75rem;
  }

  &__quick-input {
    display: grid;
    flex: 0 0 18rem;
    gap: 0.75rem;
  }

  &__dimensions {
    align-items: center;
    display: grid;
    gap: 0.5rem;
    grid-template-columns: auto 1fr auto 1fr;
  }
}
</style>
