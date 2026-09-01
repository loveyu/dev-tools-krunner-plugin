import type { ImageDimensions, OutputImageType } from './types';

import { COMPRESSION_IMAGE_TYPES } from './types';

export const MAX_COMPRESSION_IMAGE_BYTES = 25 * 1024 * 1024;
export const MAX_OUTPUT_DIMENSION = 8192;
export const MAX_OUTPUT_PIXELS = 40_000_000;

export function validateCompressionFileMetadata(type: string, size: number): string | null {
  if (!(COMPRESSION_IMAGE_TYPES as readonly string[]).includes(type)) {
    return 'ui.onlyPngJpegWebpBmpAndGifImagesAreSupported';
  }
  if (size <= 0) return 'ui.theImageIsEmpty';
  if (size > MAX_COMPRESSION_IMAGE_BYTES) return 'ui.theImageMustNotExceed25Mib';
  return null;
}

export function initialTargetDimensions(width: number, height: number): ImageDimensions {
  assertPositiveDimensions(width, height);
  const scale = Math.min(
    1,
    MAX_OUTPUT_DIMENSION / width,
    MAX_OUTPUT_DIMENSION / height,
    Math.sqrt(MAX_OUTPUT_PIXELS / (width * height)),
  );
  return scaledDimensions(width, height, scale);
}

export function containedDimensions(
  sourceWidth: number,
  sourceHeight: number,
  maxWidth: number,
  maxHeight: number,
): ImageDimensions {
  assertPositiveDimensions(sourceWidth, sourceHeight);
  assertPositiveDimensions(maxWidth, maxHeight);
  if (maxWidth > MAX_OUTPUT_DIMENSION || maxHeight > MAX_OUTPUT_DIMENSION) {
    throw new Error('convert.errors.dimensionsTooLarge');
  }
  const scale = Math.min(
    1,
    maxWidth / sourceWidth,
    maxHeight / sourceHeight,
    Math.sqrt(MAX_OUTPUT_PIXELS / (sourceWidth * sourceHeight)),
  );
  return scaledDimensions(sourceWidth, sourceHeight, scale);
}

export function encoderQuality(qualityPercent: number): number {
  if (!Number.isFinite(qualityPercent)) return 0.82;
  return Math.min(100, Math.max(1, qualityPercent)) / 100;
}

export function defaultOutputType(inputType: string): OutputImageType {
  if (inputType === 'image/jpeg') return 'image/jpeg';
  if (inputType === 'image/webp') return 'image/webp';
  return 'image/webp';
}

export function outputFilename(inputName: string, mimeType: OutputImageType): string {
  const lastDot = inputName.lastIndexOf('.');
  const baseName = lastDot > 0 ? inputName.slice(0, lastDot) : inputName;
  return `${baseName || 'image'}.compressed.${extensionForType(mimeType)}`;
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${String(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
}

/** 压缩前后体积变化：百分比文本与方向，具体文案由调用方做 i18n 渲染。 */
export type SizeDelta = { readonly percentage: string; readonly smaller: boolean };

export function sizeDelta(originalBytes: number, outputBytes: number): SizeDelta | null {
  if (originalBytes <= 0) return null;
  const percentage = Math.abs((1 - outputBytes / originalBytes) * 100).toFixed(1);
  return { percentage, smaller: outputBytes <= originalBytes };
}

function extensionForType(mimeType: OutputImageType): string {
  switch (mimeType) {
    case 'image/jpeg':
      return 'jpg';
    case 'image/webp':
      return 'webp';
    case 'image/png':
      return 'png';
  }
}

function assertPositiveDimensions(width: number, height: number): void {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    throw new Error('ui.imageDimensionsMustBePositive');
  }
}

function scaledDimensions(width: number, height: number, scale: number): ImageDimensions {
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}
