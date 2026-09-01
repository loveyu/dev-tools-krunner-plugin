import { containedDimensions, encoderQuality, validateCompressionFileMetadata } from './model';
import type { CompressionOptions, CompressionResult, ImageDimensions } from './types';

export async function inspectImage(file: File): Promise<ImageDimensions> {
  const image = await loadImage(file);
  return { width: image.naturalWidth, height: image.naturalHeight };
}

export async function compressImage(
  file: File,
  options: CompressionOptions,
): Promise<CompressionResult> {
  const validationError = validateCompressionFileMetadata(file.type, file.size);
  if (validationError !== null) throw new Error(validationError);

  const image = await loadImage(file);
  const dimensions = containedDimensions(
    image.naturalWidth,
    image.naturalHeight,
    options.maxWidth,
    options.maxHeight,
  );
  const canvas = document.createElement('canvas');
  canvas.width = dimensions.width;
  canvas.height = dimensions.height;
  const context = canvas.getContext('2d');
  if (context === null) throw new Error('errors.canvasUnavailable');

  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = 'high';
  if (options.mimeType === 'image/jpeg') {
    context.fillStyle = '#ffffff';
    context.fillRect(0, 0, canvas.width, canvas.height);
  }
  context.drawImage(image, 0, 0, canvas.width, canvas.height);

  const blob = await encodeCanvas(canvas, options);
  if (blob.type !== options.mimeType) {
    throw new Error(`当前 WebView 不支持 ${options.mimeType} 编码`);
  }
  return { blob, mimeType: options.mimeType, ...dimensions };
}

async function loadImage(file: File): Promise<HTMLImageElement> {
  const objectUrl = URL.createObjectURL(file);
  try {
    const image = new Image();
    image.decoding = 'async';
    image.src = objectUrl;
    await new Promise<void>((resolve, reject) => {
      image.onload = (): void => {
        resolve();
      };
      image.onerror = (): void => {
        reject(new Error('errors.imageDecodeFailed'));
      };
    });
    return image;
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}

async function encodeCanvas(canvas: HTMLCanvasElement, options: CompressionOptions): Promise<Blob> {
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (blob === null) {
          reject(new Error('errors.imageEncodeFailed'));
          return;
        }
        resolve(blob);
      },
      options.mimeType,
      encoderQuality(options.qualityPercent),
    );
  });
}
