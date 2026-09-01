import { describe, expect, it } from 'vitest';

import {
  WATERMARKER_PROJECT_URL,
  DEFAULT_TIME_TEMPLATE,
  canvasFontSpec,
  clampNumber,
  createDefaultWatermarkSettings,
  createTileOrigins,
  degreesToRadians,
  expandTimeTemplate,
  previewScale,
  splitWatermarkLines,
  watermarkedFilename,
} from './model';

describe('watermark defaults', () => {
  it('matches the original project defaults', () => {
    const settings = createDefaultWatermarkSettings('demo');
    expect(settings.opacity).toBe(0.15);
    expect(settings.angle).toBe(-45);
    expect(settings.scale).toBe(1);
    expect(settings.textStyle.fontSize).toBe(24);
    expect(settings.textStyle.fontWeight).toBe(400);
    expect(settings.textStyle.center).toBe(true);
    expect(settings.textStyle.textColor).toBe('#ffffff');
    expect(settings.gapX).toBe(100);
    expect(settings.gapY).toBe(75);
    expect(settings.offsetX).toBe(0);
    expect(settings.offsetY).toBe(0);
    expect(settings.quality).toBe(0.9);
  });

  it('keeps the original project attribution URL stable', () => {
    expect(WATERMARKER_PROJECT_URL).toBe('https://github.com/TransparentLC/watermarker');
    expect(DEFAULT_TIME_TEMPLATE).toBe('{Y}-{M}-{D} {h}:{m}:{s}');
  });
});

describe('clampNumber', () => {
  it('clamps finite values and falls back for invalid input', () => {
    expect(clampNumber(5, 0, 1, 0.5)).toBe(1);
    expect(clampNumber(-5, 0, 1, 0.5)).toBe(0);
    expect(clampNumber(0.5, 0, 1, 0.5)).toBe(0.5);
    expect(clampNumber(Number.NaN, 0, 1, 0.5)).toBe(0.5);
    expect(clampNumber(Number.POSITIVE_INFINITY, 0, 1, 0.5)).toBe(1);
  });
});

describe('expandTimeTemplate', () => {
  const now = new Date(2026, 8, 1, 7, 5, 3);

  it('expands every placeholder with zero padding', () => {
    expect(expandTimeTemplate('{Y}-{M}-{D} {h}:{m}:{s}', now)).toBe('2026-09-01 07:05:03');
    expect(expandTimeTemplate('{Y}', now)).toBe('2026');
  });

  it('keeps unrelated braces and plain text untouched', () => {
    expect(expandTimeTemplate('仅用于{X}演示 {Y}', now)).toBe('仅用于{X}演示 2026');
    expect(expandTimeTemplate('没有占位符', now)).toBe('没有占位符');
  });
});

describe('splitWatermarkLines', () => {
  it('drops leading and trailing blank lines but keeps inner ones', () => {
    expect(splitWatermarkLines('\n第一行\n\n中间\n  \n尾行\n\n')).toEqual([
      '第一行',
      '',
      '中间',
      '  ',
      '尾行',
    ]);
  });

  it('expands tabs and returns empty array for blank input', () => {
    expect(splitWatermarkLines('a\tb')).toEqual(['a    b']);
    expect(splitWatermarkLines('')).toEqual([]);
    expect(splitWatermarkLines('\n \n')).toEqual([]);
  });
});

describe('createTileOrigins', () => {
  it('covers the rotated bounds and anchors the offset on the grid', () => {
    const origins = createTileOrigins(100, 80, 20, 10, 5, 5, 0, 0);
    expect(origins.length).toBeGreaterThan(20);
    expect(origins.some(({ x, y }) => x === 0 && y === 0)).toBe(true);
    // 覆盖范围应达到旋转后的对角半径（hypot(100,80)/2 ≈ 64）。
    expect(origins.some(({ x }) => x >= 64)).toBe(true);
    expect(origins.some(({ y }) => y <= -64)).toBe(true);
  });

  it('shifts the whole grid by the given offset', () => {
    const base = createTileOrigins(100, 80, 20, 10, 5, 5, 0, 0);
    const shifted = createTileOrigins(100, 80, 20, 10, 5, 5, 13, -7);
    const first = base[0];
    expect(first).toEqual({ x: -100, y: -90 });
    expect(shifted[0]).toEqual({ x: -87, y: -97 });
    expect(shifted.some(({ x, y }) => x === 13 && y === -7)).toBe(true);
  });

  it('clamps non-positive tile steps instead of looping forever', () => {
    const collapsed = createTileOrigins(100, 80, 20, 10, -1000, -1000, 0, 0);
    expect(collapsed.length).toBeGreaterThan(0);
    // 步长被钳到 1 px，所有原点仍然有限。
    expect(collapsed.every(({ x, y }) => Number.isFinite(x) && Number.isFinite(y))).toBe(true);
  });

  it('rejects invalid dimensions and layout values', () => {
    expect(() => createTileOrigins(0, 80, 20, 10, 5, 5, 0, 0)).toThrow(
      'watermark.errors.invalidDimensions',
    );
    expect(() => createTileOrigins(100, 80, -1, 10, 5, 5, 0, 0)).toThrow(
      'watermark.errors.invalidDimensions',
    );
    expect(() => createTileOrigins(100, 80, 20, 10, 5, 5, Number.NaN, 0)).toThrow(
      'watermark.errors.invalidLayout',
    );
  });
});

describe('previewScale', () => {
  it('keeps small images at full resolution and caps the longest side', () => {
    expect(previewScale(800, 600)).toBe(1);
    expect(previewScale(2800, 1400)).toBeCloseTo(0.5);
    expect(previewScale(1400, 2800)).toBeCloseTo(0.5);
  });
});

describe('watermarkedFilename', () => {
  it('creates filenames for every browser output type', () => {
    expect(watermarkedFilename('photo.jpeg', 'image/jpeg')).toBe('photo.watermarked.jpg');
    expect(watermarkedFilename('photo.png', 'image/webp')).toBe('photo.watermarked.webp');
    expect(watermarkedFilename('photo', 'image/png')).toBe('photo.watermarked.png');
    expect(watermarkedFilename('', 'image/png')).toBe('image.watermarked.png');
  });
});

describe('canvasFontSpec', () => {
  const base = createDefaultWatermarkSettings('x').textStyle;

  it('assembles italic, weight and size in CSS order', () => {
    expect(canvasFontSpec(base, 24)).toBe('400 24px sans-serif');
    expect(canvasFontSpec({ ...base, italic: true, fontWeight: 700 }, 12.5)).toBe(
      'italic 700 12.5px sans-serif',
    );
    expect(canvasFontSpec(base, 0)).toBe('400 1px sans-serif');
  });
});

describe('degreesToRadians', () => {
  it('converts degrees to radians', () => {
    expect(degreesToRadians(180)).toBeCloseTo(Math.PI);
    expect(degreesToRadians(-45)).toBeCloseTo(-Math.PI / 4);
  });
});
