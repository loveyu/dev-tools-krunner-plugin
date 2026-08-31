import type { PreparedImage } from './types';

export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
export const SUPPORTED_IMAGE_TYPES = [
  'image/png',
  'image/jpeg',
  'image/bmp',
  'image/tiff',
  'image/webp',
  'image/gif',
] as const;

export function validateImageMetadata(type: string, size: number): string | null {
  if (!(SUPPORTED_IMAGE_TYPES as readonly string[]).includes(type)) {
    return '仅支持 PNG、JPEG、BMP、TIFF、WebP 和 GIF 图片';
  }
  if (size <= 0) return '图片内容为空';
  if (size > MAX_IMAGE_BYTES) return '图片不能超过 10 MiB';
  return null;
}

export async function prepareImage(file: File): Promise<PreparedImage> {
  const validationError = validateImageMetadata(file.type, file.size);
  if (validationError !== null) throw new Error(validationError);
  const bytes = new Uint8Array(await file.arrayBuffer());
  return {
    imageBase64: bytesToBase64(bytes),
    mimeType: file.type,
  };
}

export function bytesToBase64(bytes: Uint8Array): string {
  const chunkSize = 0x8000;
  const chunks: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const chunk = bytes.subarray(offset, offset + chunkSize);
    chunks.push(String.fromCharCode(...chunk));
  }
  return btoa(chunks.join(''));
}

export function firstImageFile(files: FileList | readonly File[]): File | null {
  for (const file of Array.from(files)) {
    if (file.type.startsWith('image/')) return file;
  }
  return null;
}
