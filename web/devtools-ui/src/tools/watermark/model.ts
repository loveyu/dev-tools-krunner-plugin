import type { OutputImageType } from '../image-compression/types';

export const WATERMARKER_PROJECT_URL = 'https://github.com/TransparentLC/watermarker';

export type TileOrigin = {
  readonly x: number;
  readonly y: number;
};

export function normalizePercentage(value: number, fallback: number): number {
  return Number.isFinite(value) ? Math.min(100, Math.max(1, value)) : fallback;
}

export function degreesToRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

export function createTileOrigins(
  canvasWidth: number,
  canvasHeight: number,
  markWidth: number,
  markHeight: number,
  horizontalGap: number,
  verticalGap: number,
): readonly TileOrigin[] {
  const values = [canvasWidth, canvasHeight, markWidth, markHeight];
  if (values.some((value) => !Number.isFinite(value) || value <= 0)) {
    throw new Error('watermark dimensions must be positive');
  }
  if (horizontalGap < 0 || verticalGap < 0) {
    throw new Error('watermark gaps cannot be negative');
  }

  const radius = Math.ceil(Math.hypot(canvasWidth, canvasHeight) / 2);
  const stepX = markWidth + horizontalGap;
  const stepY = markHeight + verticalGap;
  const origins: TileOrigin[] = [];
  for (let y = -radius - markHeight; y <= radius + markHeight; y += stepY) {
    for (let x = -radius - markWidth; x <= radius + markWidth; x += stepX) {
      origins.push({ x, y });
    }
  }
  return origins;
}

export function watermarkedFilename(inputName: string, mimeType: OutputImageType): string {
  const lastDot = inputName.lastIndexOf('.');
  const baseName = lastDot > 0 ? inputName.slice(0, lastDot) : inputName;
  return `${baseName || 'image'}.watermarked.${extensionForType(mimeType)}`;
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
