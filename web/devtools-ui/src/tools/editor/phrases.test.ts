import { describe, expect, it } from 'vitest';

import { codemirrorPhrases } from './phrases';

describe('codemirrorPhrases', () => {
  it('简体中文返回短语表', () => {
    const phrases = codemirrorPhrases('zh-CN');
    expect(phrases).toBeDefined();
    expect(phrases?.['Find']).toBe('查找');
    expect(phrases?.['replace all']).toBe('全部替换');
  });

  it('繁体中文与简体中文的 key 完全一致', () => {
    const simplified = codemirrorPhrases('zh-CN');
    const traditional = codemirrorPhrases('zh-TW');
    expect(traditional).toBeDefined();
    expect(Object.keys(traditional ?? {}).sort()).toEqual(Object.keys(simplified ?? {}).sort());
  });

  it('英语使用 CodeMirror 默认文案', () => {
    expect(codemirrorPhrases('en-US')).toBeUndefined();
  });
});
