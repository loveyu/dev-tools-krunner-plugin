<script setup lang="ts">
import { computed } from 'vue';
import { oneDark } from '@codemirror/theme-one-dark';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { Codemirror } from 'vue-codemirror';

import { useI18n } from '../i18n/runtime';
import { effectiveThemeDark } from '../theme/runtime';
import { codemirrorPhrases } from '../tools/editor/phrases';
import { languageExtension } from '../tools/editor/languages';
import type { EditorLanguageId } from '../tools/editor/languages';

defineOptions({ name: 'CodeEditor' });

const props = withDefaults(
  defineProps<{
    readonly language?: EditorLanguageId;
    // 显式允许 undefined，配合 exactOptionalPropertyTypes 表示“不限制最大高度”。
    readonly maxHeight?: string | undefined;
    readonly minHeight?: string;
    readonly modelValue: string;
    readonly placeholder?: string;
    readonly readonly?: boolean;
  }>(),
  {
    language: 'plain',
    maxHeight: undefined,
    minHeight: '10rem',
    placeholder: '',
    readonly: false,
  },
);

const emit = defineEmits<{
  'update:modelValue': [value: string];
}>();

const { locale } = useI18n();
const isDark = effectiveThemeDark();

// 透明底 + 主题 CSS 变量，让编辑器融入所在面板；必须排在 oneDark 之后才能覆盖其底色。
const editorChrome = EditorView.theme({
  '&': {
    backgroundColor: 'transparent',
    color: 'inherit',
    fontFamily: 'var(--font-code)',
    fontSize: '0.85rem',
    height: '100%',
  },
  '&.cm-focused': {
    outline: 'none',
  },
  '.cm-activeLine': {
    backgroundColor: 'color-mix(in srgb, currentColor 6%, transparent)',
  },
  '.cm-activeLineGutter': {
    backgroundColor: 'color-mix(in srgb, currentColor 8%, transparent)',
  },
  '.cm-gutters': {
    backgroundColor: 'transparent',
    borderColor: 'var(--border-color)',
    color: 'var(--muted-color)',
  },
});

const extensions = computed<Extension[]>(() => {
  const list: Extension[] = [
    EditorView.lineWrapping,
    editorChrome,
    // 搜索面板等内置界面文案的本地化；英语传空表以维持默认。
    EditorState.phrases.of(codemirrorPhrases(locale.value) ?? {}),
  ];
  const language = languageExtension(props.language);
  if (language !== null) list.push(language);
  // 只读输出保留 editable，使内容仍可聚焦、全选和复制。
  if (props.readonly) list.push(EditorState.readOnly.of(true));
  if (isDark.value) list.push(oneDark);
  return list;
});

function handleInput(value: string): void {
  emit('update:modelValue', value);
}
</script>

<template>
  <!-- 外层盒子承担高度与边框；vue-codemirror 的根节点是 display:contents，不能承载样式。 -->
  <div class="code-editor" :style="{ maxHeight: maxHeight, minHeight: minHeight }">
    <Codemirror
      :extensions="extensions"
      :model-value="modelValue"
      :placeholder="placeholder"
      :tab-size="2"
      @update:model-value="handleInput"
    />
  </div>
</template>

<style scoped lang="scss">
.code-editor {
  border: 1px solid var(--border-color);
  border-radius: 3px;
  display: flex;
  flex-direction: column;
  min-width: 0;

  :deep(.cm-editor) {
    // 父盒子高度受限时填满并在内部滚动；高度自适应时随内容增长。
    flex: 1 1 auto;
    min-height: 0;
  }
}
</style>
