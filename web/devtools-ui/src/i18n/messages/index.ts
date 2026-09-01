import type { SupportedLocale } from '../core';

import { english } from './en-US';
import { simplifiedChinese } from './zh-CN';
import { traditionalChinese } from './zh-TW';

export const MESSAGES: Readonly<Record<SupportedLocale, Readonly<Record<string, string>>>> = {
  'zh-CN': simplifiedChinese,
  'zh-TW': traditionalChinese,
  'en-US': english,
};
