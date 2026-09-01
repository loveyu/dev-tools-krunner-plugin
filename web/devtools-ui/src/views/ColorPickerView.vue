<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { NAlert, NButton, NCard, NInput, NSpace, NTag, useMessage } from 'naive-ui';

import { useI18n } from '../i18n/runtime';
import { postRequest } from '../ipc/bridge';
import { pickScreenColor } from '../ipc/native-color';
import { colorFromHex, colorFromHsv, colorFromRgb, type ColorFormats } from '../tools/color/model';

defineOptions({ name: 'ColorPickerView' });

const HISTORY_KEY = 'devtools.color.history.v1';
const { t } = useI18n();
const message = useMessage();
const hue = ref(210);
const saturation = ref(0.7);
const value = ref(0.85);
const color = ref<ColorFormats>(colorFromHsv(hue.value, saturation.value, value.value));
const hexInput = ref(color.value.hex);
const history = ref<string[]>([]);
const picking = ref(false);
const error = ref<string | null>(null);
const boardStyle = computed(() => ({
  backgroundColor: `hsl(${String(hue.value)} 100% 50%)`,
}));
const handleStyle = computed(() => ({
  left: `${String(saturation.value * 100)}%`,
  top: `${String((1 - value.value) * 100)}%`,
  backgroundColor: color.value.hex,
}));

onMounted(() => {
  try {
    const stored = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? '[]') as unknown;
    if (Array.isArray(stored)) {
      history.value = stored.filter((item): item is string => colorFromHex(String(item)) !== null);
    }
  } catch {
    history.value = [];
  }
});

function updateBoard(event: PointerEvent): void {
  const board = event.currentTarget;
  if (!(board instanceof HTMLElement)) return;
  const bounds = board.getBoundingClientRect();
  saturation.value = Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width));
  value.value = 1 - Math.min(1, Math.max(0, (event.clientY - bounds.top) / bounds.height));
  applyColor(colorFromHsv(hue.value, saturation.value, value.value), false);
}

function handleHueInput(event: Event): void {
  const input = event.currentTarget;
  if (!(input instanceof HTMLInputElement)) return;
  hue.value = Number(input.value);
  applyColor(colorFromHsv(hue.value, saturation.value, value.value), false);
}

function handleHexChange(): void {
  const next = colorFromHex(hexInput.value);
  if (next === null) {
    error.value = t('color.errors.invalidHex');
    return;
  }
  error.value = null;
  applyColor(next, true);
}

async function pickFromScreen(): Promise<void> {
  // 屏幕取色进行中不允许重复触发，避免并发 IPC 与状态互相覆盖。
  if (picking.value) return;
  picking.value = true;
  error.value = null;
  try {
    const picked = await pickScreenColor();
    if (picked !== null) {
      applyColor(colorFromRgb(picked.red, picked.green, picked.blue), true);
      message.success(t('color.messages.picked'));
    }
  } catch (caught) {
    error.value = t(caught instanceof Error ? caught.message : String(caught));
  } finally {
    picking.value = false;
  }
}

function applyColor(next: ColorFormats, remember: boolean): void {
  color.value = next;
  hexInput.value = next.hex;
  if (remember) rememberColor(next.hex);
}

function rememberColor(hex: string): void {
  history.value = [hex, ...history.value.filter((item) => item !== hex)].slice(0, 12);
  localStorage.setItem(HISTORY_KEY, JSON.stringify(history.value));
}

function selectHistory(hex: string): void {
  const next = colorFromHex(hex);
  if (next !== null) applyColor(next, true);
}

function copy(value: string): void {
  rememberColor(color.value.hex);
  if (postRequest({ type: 'clipboardWrite', text: value })) {
    message.success(t('color.messages.copied'));
  }
}
</script>

<template>
  <main class="color-view">
    <header>
      <div>
        <h1>{{ t('color.title') }}</h1>
        <p>{{ t('color.description') }}</p>
      </div>
      <NButton type="primary" :loading="picking" @click="pickFromScreen">
        {{ t('color.actions.pickScreen') }}
      </NButton>
    </header>

    <NAlert type="info" :show-icon="false">{{ t('color.screenHint') }}</NAlert>
    <NAlert v-if="error !== null" type="error">{{ error }}</NAlert>

    <section class="color-view__workspace">
      <NCard :title="t('color.board.title')" :bordered="false">
        <div
          class="color-view__board"
          :style="boardStyle"
          @pointerdown="updateBoard"
          @pointermove.left="updateBoard"
        >
          <span class="color-view__handle" :style="handleStyle" />
        </div>
        <input
          class="color-view__hue"
          type="range"
          min="0"
          max="359"
          :value="hue"
          :aria-label="t('color.fields.hue')"
          @input="handleHueInput"
        />
      </NCard>

      <NCard :title="t('color.result.title')" :bordered="false">
        <div class="color-view__preview" :style="{ backgroundColor: color.hex }" />
        <NSpace vertical>
          <div class="color-view__format">
            <NInput v-model:value="hexInput" @change="handleHexChange" />
            <NButton @click="copy(color.hex)">{{ t('ui.copy') }}</NButton>
          </div>
          <div class="color-view__format">
            <code>{{ color.rgb }}</code>
            <NButton @click="copy(color.rgb)">{{ t('ui.copy') }}</NButton>
          </div>
          <div class="color-view__format">
            <code>{{ color.hsl }}</code>
            <NButton @click="copy(color.hsl)">{{ t('ui.copy') }}</NButton>
          </div>
        </NSpace>
      </NCard>
    </section>

    <NCard v-if="history.length > 0" :title="t('color.history.title')" :bordered="false">
      <NSpace>
        <NTag
          v-for="item in history"
          :key="item"
          class="color-view__history"
          :style="{ borderColor: item }"
          @click="selectHistory(item)"
        >
          <span class="color-view__swatch" :style="{ backgroundColor: item }" />{{ item }}
        </NTag>
      </NSpace>
    </NCard>
  </main>
</template>

<style scoped lang="scss">
.color-view {
  display: grid;
  gap: var(--page-gap);
  height: var(--app-viewport-height);
  min-height: 0;
  overflow: auto;
  padding: var(--page-padding);
  padding-inline: max(var(--page-padding), calc((100% - 68rem) / 2));

  header {
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

  &__workspace {
    display: grid;
    gap: 1rem;
    grid-template-columns: minmax(0, 1.35fr) minmax(18rem, 0.65fr);
  }

  &__board {
    background-image:
      linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, transparent);
    border-radius: 0.5rem;
    cursor: crosshair;
    height: clamp(14rem, 44vh, 22rem);
    position: relative;
    touch-action: none;
  }

  &__handle {
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px #000;
    height: 1rem;
    position: absolute;
    transform: translate(-50%, -50%);
    width: 1rem;
  }

  &__hue {
    accent-color: #2563eb;
    margin-top: 1rem;
    width: 100%;
  }

  &__preview {
    border: 1px solid var(--border-color);
    border-radius: 0.5rem;
    height: clamp(6rem, 20vh, 10rem);
    margin-bottom: 1rem;
  }

  &__format {
    align-items: center;
    display: grid;
    gap: 0.5rem;
    grid-template-columns: minmax(0, 1fr) auto;

    code {
      overflow-wrap: anywhere;
    }
  }

  &__history {
    cursor: pointer;
  }

  &__swatch {
    border-radius: 50%;
    display: inline-block;
    height: 0.8rem;
    margin-right: 0.35rem;
    vertical-align: -0.08rem;
    width: 0.8rem;
  }
}

@media (width <= 820px) {
  .color-view {
    header {
      align-items: stretch;
      flex-direction: column;
    }

    &__workspace {
      grid-template-columns: 1fr;
    }
  }
}

@media (height <= 620px) and (width > 820px) {
  .color-view {
    &__board {
      height: 13rem;
    }

    &__preview {
      height: 6rem;
    }
  }
}
</style>
