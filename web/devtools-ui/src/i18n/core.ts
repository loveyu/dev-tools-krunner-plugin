import type { LanguageMode } from '../ipc/types';
import { MESSAGES } from './messages';

export type SupportedLocale = 'zh-CN' | 'zh-TW' | 'en-US';
export type TranslationParameters = Readonly<Record<string, number | string>>;
export type Translator = (key: string, parameters?: TranslationParameters) => string;

export function detectLocale(languages: readonly string[]): SupportedLocale {
  for (const language of languages) {
    const normalized = language.toLowerCase();
    if (normalized.startsWith('zh')) {
      return /(?:tw|hk|mo|hant)/u.test(normalized) ? 'zh-TW' : 'zh-CN';
    }
    if (normalized.startsWith('en')) return 'en-US';
  }
  return 'en-US';
}

export function initialSystemLocale(
  platformLocale: SupportedLocale | undefined,
  browserLanguages: readonly string[],
): SupportedLocale {
  return platformLocale ?? detectLocale(browserLanguages);
}

export function resolveLocale(mode: LanguageMode, systemLocale: SupportedLocale): SupportedLocale {
  return mode === 'system' ? systemLocale : mode;
}

export function translate(
  locale: SupportedLocale,
  key: string,
  parameters?: TranslationParameters,
): string {
  const template = MESSAGES[locale][key] ?? key;
  if (parameters === undefined) return template;
  return template.replace(/\{([a-zA-Z][a-zA-Z0-9]*)\}/gu, (match, name: string) => {
    const value = parameters[name];
    return value === undefined ? match : String(value);
  });
}
