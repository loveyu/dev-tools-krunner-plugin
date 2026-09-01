import type { ComputedRef, InjectionKey, Ref } from 'vue';
import { inject, provide } from 'vue';

import { translate } from './core';
import type { SupportedLocale, Translator } from './core';

export { detectLocale, initialSystemLocale, resolveLocale, translate } from './core';
export type { SupportedLocale, TranslationParameters, Translator } from './core';

type I18nContext = {
  readonly locale: Readonly<Ref<SupportedLocale>>;
  readonly t: Translator;
};

const I18N_KEY: InjectionKey<I18nContext> = Symbol('devtools-i18n');

export function provideI18n(locale: ComputedRef<SupportedLocale>): void {
  provide(I18N_KEY, {
    locale,
    t: (key, parameters) => translate(locale.value, key, parameters),
  });
}

export function useI18n(): I18nContext {
  const context = inject(I18N_KEY);
  if (context === undefined) throw new Error('i18n context is unavailable');
  return context;
}
