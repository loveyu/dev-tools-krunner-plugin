import { describe, expect, it } from 'vitest';

import { colorFromHex, colorFromHsv, colorFromRgb, normalizeHex } from './model';

describe('color model', () => {
  it('normalizes three and six digit colors', () => {
    expect(normalizeHex(' #abc ')).toBe('#AABBCC');
    expect(normalizeHex('00ff7f')).toBe('#00FF7F');
    expect(normalizeHex('nope')).toBeNull();
    expect(colorFromHex('#bad-value')).toBeNull();
  });

  it('formats RGB and clamps channels', () => {
    expect(colorFromRgb(-1, 127.6, 300)).toEqual({
      hex: '#0080FF',
      red: 0,
      green: 128,
      blue: 255,
      rgb: 'rgb(0, 128, 255)',
      hsl: 'hsl(210, 100%, 50%)',
    });
    expect(colorFromHex('#fff')?.hsl).toBe('hsl(0, 0%, 100%)');
  });

  it('converts every HSV sector and HSL maximum branch', () => {
    expect([0, 60, 120, 180, 240, 300].map((hue) => colorFromHsv(hue, 1, 1).hex)).toEqual([
      '#FF0000',
      '#FFFF00',
      '#00FF00',
      '#00FFFF',
      '#0000FF',
      '#FF00FF',
    ]);
    expect(colorFromHsv(-60, -1, 2).hex).toBe('#FFFFFF');
    expect(colorFromRgb(128, 255, 0).hsl).toBe('hsl(90, 100%, 50%)');
    expect(colorFromRgb(0, 128, 255).hsl).toBe('hsl(210, 100%, 50%)');
  });
});
