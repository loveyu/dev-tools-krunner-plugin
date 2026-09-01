import type { SupportedLocale } from '../../i18n/core';

// CodeMirror 内置界面文案（搜索面板等）的本地化映射；key 为 CodeMirror 官方英文短语。
const SIMPLIFIED_CHINESE_PHRASES: Readonly<Record<string, string>> = {
  'Go to line': '跳转到行',
  go: '跳转',
  Find: '查找',
  Replace: '替换',
  next: '下一个',
  previous: '上一个',
  all: '全部',
  'match case': '区分大小写',
  regexp: '正则表达式',
  'by word': '按单词',
  replace: '替换',
  'replace all': '全部替换',
  close: '关闭',
  'current match': '当前匹配',
  'replaced $ matches': '已替换 $ 处匹配',
  'replaced match on line $': '已替换第 $ 行的匹配',
  'on line': '于行',
};

const TRADITIONAL_CHINESE_PHRASES: Readonly<Record<string, string>> = {
  'Go to line': '跳轉到行',
  go: '跳轉',
  Find: '尋找',
  Replace: '取代',
  next: '下一個',
  previous: '上一個',
  all: '全部',
  'match case': '區分大小寫',
  regexp: '正規表示式',
  'by word': '按單詞',
  replace: '取代',
  'replace all': '全部取代',
  close: '關閉',
  'current match': '目前匹配',
  'replaced $ matches': '已取代 $ 處匹配',
  'replaced match on line $': '已取代第 $ 行的匹配',
  'on line': '於行',
};

/** 返回指定语言的 CodeMirror 界面短语；英语返回 undefined 以使用内置默认值。 */
export function codemirrorPhrases(locale: SupportedLocale): Record<string, string> | undefined {
  if (locale === 'zh-CN') return { ...SIMPLIFIED_CHINESE_PHRASES };
  if (locale === 'zh-TW') return { ...TRADITIONAL_CHINESE_PHRASES };
  return undefined;
}
