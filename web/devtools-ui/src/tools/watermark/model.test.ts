import { describe, expect, it } from 'vitest';

import {
  WATERMARKER_PROJECT_URL,
  createTileOrigins,
  degreesToRadians,
  normalizePercentage,
  watermarkedFilename,
} from './model';

describe('watermark settings helpers', () => {
  it('normalizes percentages and angles', () => {
    expect(normalizePercentage(Number.NaN, 32)).toBe(32);
    expect(normalizePercentage(0, 32)).toBe(1);
    expect(normalizePercentage(45, 32)).toBe(45);
    expect(normalizePercentage(200, 32)).toBe(100);
    expect(degreesToRadians(180)).toBeCloseTo(Math.PI);
  });

  it('keeps the original project attribution URL stable', () => {
    expect(WATERMARKER_PROJECT_URL).toBe('https://github.com/TransparentLC/watermarker');
  });
});

describe('createTileOrigins', () => {
  it('covers the full rotated image bounds with a stable grid', () => {
    const origins = createTileOrigins(100, 80, 20, 10, 5, 5);
    expect(origins.length).toBeGreaterThan(20);
    expect(origins[0]).toEqual({ x: -85, y: -75 });
    expect(origins.some(({ x, y }) => x >= 65 && y >= 65)).toBe(true);
  });

  it('rejects invalid dimensions and gaps', () => {
    expect(() => createTileOrigins(0, 80, 20, 10, 5, 5)).toThrow('positive');
    expect(() => createTileOrigins(100, 80, 20, 10, -1, 5)).toThrow('negative');
    expect(() => createTileOrigins(100, 80, 20, 10, 5, -1)).toThrow('negative');
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
