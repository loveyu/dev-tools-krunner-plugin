export const COMPRESSION_IMAGE_TYPES = [
  'image/png',
  'image/jpeg',
  'image/webp',
  'image/bmp',
  'image/gif',
] as const;

export const OUTPUT_IMAGE_TYPES = ['image/jpeg', 'image/webp', 'image/png'] as const;

export type OutputImageType = (typeof OUTPUT_IMAGE_TYPES)[number];

export type ImageDimensions = {
  readonly width: number;
  readonly height: number;
};

export type CompressionOptions = {
  readonly mimeType: OutputImageType;
  readonly qualityPercent: number;
  readonly maxWidth: number;
  readonly maxHeight: number;
};

export type CompressionResult = ImageDimensions & {
  readonly blob: Blob;
  readonly mimeType: OutputImageType;
};
