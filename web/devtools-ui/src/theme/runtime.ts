import { readonly, ref } from 'vue';
import type { Ref } from 'vue';

// 当前实际生效的明暗主题。App.vue 在主题设置或系统配色变化时写入；
// Naive UI 之外的组件（CodeMirror 编辑器等）读取它来切换语法高亮配色。
const dark = ref(false);

export function setEffectiveTheme(isDark: boolean): void {
  dark.value = isDark;
}

export function effectiveThemeDark(): Readonly<Ref<boolean>> {
  return readonly(dark);
}
