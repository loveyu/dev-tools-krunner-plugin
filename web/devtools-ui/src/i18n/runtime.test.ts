import { describe, expect, it } from 'vitest';

import { MESSAGES } from './messages';
import { detectLocale, resolveLocale, translate } from './core';

describe('i18n runtime', () => {
  it('detects simplified and traditional Chinese variants', () => {
    expect(detectLocale(['zh-CN'])).toBe('zh-CN');
    expect(detectLocale(['zh-Hans'])).toBe('zh-CN');
    expect(detectLocale(['zh-TW'])).toBe('zh-TW');
    expect(detectLocale(['zh-Hant-HK'])).toBe('zh-TW');
  });

  it('detects English and falls back to English for unsupported languages', () => {
    expect(detectLocale(['fr-FR', 'en-GB'])).toBe('en-US');
    expect(detectLocale(['de-DE'])).toBe('en-US');
  });

  it('resolves automatic and explicitly selected modes', () => {
    expect(resolveLocale('system', ['zh-TW'])).toBe('zh-TW');
    expect(resolveLocale('zh-CN', ['en-US'])).toBe('zh-CN');
    expect(resolveLocale('zh-TW', ['en-US'])).toBe('zh-TW');
    expect(resolveLocale('en-US', ['zh-CN'])).toBe('en-US');
  });

  it('translates, interpolates and falls back to the source key', () => {
    expect(translate('en-US', 'ui.chooseImage')).toBe('Choose image');
    expect(translate('zh-TW', 'ui.countMatches', { count: 3 })).toBe('3 個命中');
    expect(translate('en-US', 'Unknown {value}', {})).toBe('Unknown {value}');
    expect(translate('zh-CN', 'ui.chooseImage')).toBe('选择图片');
  });

  it('keeps all locale keys aligned and free of Chinese source text', () => {
    expect(Object.keys(MESSAGES['zh-CN']).sort()).toEqual(Object.keys(MESSAGES['en-US']).sort());
    expect(Object.keys(MESSAGES['zh-TW']).sort()).toEqual(Object.keys(MESSAGES['en-US']).sort());
    expect(Object.keys(MESSAGES['en-US']).every((key) => !/\p{Script=Han}/u.test(key))).toBe(true);
  });
});
