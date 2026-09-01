import { describe, expect, it } from 'vitest';

import {
  MAX_COMPRESSION_IMAGE_BYTES,
  containedDimensions,
  defaultOutputType,
  encoderQuality,
  formatFileSize,
  initialTargetDimensions,
  outputFilename,
  sizeDeltaLabel,
  validateCompressionFileMetadata,
} from './model';

describe('validateCompressionFileMetadata', () => {
  it('accepts supported non-empty images within the limit', () => {
    expect(validateCompressionFileMetadata('image/png', 128)).toBeNull();
  });

  it('rejects unsupported, empty and oversized images', () => {
    expect(validateCompressionFileMetadata('image/tiff', 128)).toContain(
      'ui.onlyPngJpegWebpBmpAndGifImagesAreSupported',
    );
    expect(validateCompressionFileMetadata('image/png', 0)).toContain('ui.theImageIsEmpty');
    expect(validateCompressionFileMetadata('image/png', MAX_COMPRESSION_IMAGE_BYTES + 1)).toContain(
      'ui.theImageMustNotExceed25Mib',
    );
  });
});

describe('dimensions', () => {
  it('keeps ordinary images at their source size', () => {
    expect(initialTargetDimensions(1600, 900)).toEqual({ width: 1600, height: 900 });
    expect(containedDimensions(1600, 900, 1600, 900)).toEqual({ width: 1600, height: 900 });
  });

  it('limits dimensions and total pixel count while preserving ratio', () => {
    expect(initialTargetDimensions(16_000, 8_000)).toEqual({ width: 8192, height: 4096 });
    expect(initialTargetDimensions(8000, 8000)).toEqual({ width: 6325, height: 6325 });
    expect(containedDimensions(4000, 2000, 1000, 1000)).toEqual({
      width: 1000,
      height: 500,
    });
  });

  it('rejects invalid or excessive dimensions', () => {
    expect(() => initialTargetDimensions(0, 100)).toThrow('ui.imageDimensionsMustBePositive');
    expect(() => containedDimensions(100, 100, Number.NaN, 100)).toThrow(
      'ui.imageDimensionsMustBePositive',
    );
    expect(() => containedDimensions(100, 100, 9000, 100)).toThrow(
      'convert.errors.dimensionsTooLarge',
    );
  });

  it('never rounds a scaled side down to zero', () => {
    expect(containedDimensions(1, 100_000, 1, 1)).toEqual({ width: 1, height: 1 });
  });
});

describe('compression presentation helpers', () => {
  it('normalizes encoder quality', () => {
    expect(encoderQuality(Number.NaN)).toBe(0.82);
    expect(encoderQuality(0)).toBe(0.01);
    expect(encoderQuality(82)).toBe(0.82);
    expect(encoderQuality(200)).toBe(1);
  });

  it('chooses a browser output type', () => {
    expect(defaultOutputType('image/jpeg')).toBe('image/jpeg');
    expect(defaultOutputType('image/webp')).toBe('image/webp');
    expect(defaultOutputType('image/png')).toBe('image/webp');
  });

  it('builds stable output filenames for every format', () => {
    expect(outputFilename('photo.jpeg', 'image/jpeg')).toBe('photo.compressed.jpg');
    expect(outputFilename('archive.image.png', 'image/webp')).toBe('archive.image.compressed.webp');
    expect(outputFilename('.hidden', 'image/png')).toBe('.hidden.compressed.png');
    expect(outputFilename('', 'image/png')).toBe('image.compressed.png');
  });

  it('formats byte sizes across units', () => {
    expect(formatFileSize(512)).toBe('512 B');
    expect(formatFileSize(1536)).toBe('1.5 KiB');
    expect(formatFileSize(2 * 1024 * 1024)).toBe('2.00 MiB');
  });

  it('describes smaller, larger and unavailable size deltas', () => {
    expect(sizeDeltaLabel(1000, 750)).toBe('减少 25.0%');
    expect(sizeDeltaLabel(1000, 1100)).toBe('增加 10.0%');
    expect(sizeDeltaLabel(0, 0)).toBe('—');
  });
});
