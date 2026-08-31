import { describe, expect, it } from 'vitest';

import { toastEditorLocale, TOAST_EDITOR_THEME } from './config';

describe('TOAST UI image editor configuration', () => {
  it('provides complete Chinese menu and filter labels', () => {
    const simplified = toastEditorLocale('zh-CN');
    const traditional = toastEditorLocale('zh-TW');
    const requiredKeys = [
      'Resize',
      'Crop',
      'Flip',
      'Rotate',
      'Draw',
      'Shape',
      'Icon',
      'Text',
      'Filter',
      'Undo',
      'Redo',
      'Color Filter',
      'Custom icon',
      'Load Mask Image',
    ];

    for (const key of requiredKeys) {
      expect(simplified[key]).toBeTypeOf('string');
      expect(traditional[key]).toBeTypeOf('string');
    }
    expect(simplified['Icon']).toBe('图标');
    expect(traditional['Icon']).toBe('圖示');
  });

  it('uses the editor native labels for English', () => {
    expect(toastEditorLocale('en-US')).toEqual({});
  });

  it('uses only embedded assets and theme variables', () => {
    expect(TOAST_EDITOR_THEME['common.bi.image']).toMatch(/^data:image\/gif;base64,/u);
    expect(TOAST_EDITOR_THEME['common.backgroundColor']).toBe('var(--editor-background)');
  });
});
