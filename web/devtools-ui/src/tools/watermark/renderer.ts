import { encoderQuality, initialTargetDimensions } from '../image-compression/model';
import type { OutputImageType } from '../image-compression/types';
import { createTileOrigins, degreesToRadians, normalizePercentage } from './model';

export type WatermarkContent =
  | {
      readonly type: 'image';
      readonly file: File;
      readonly width: number;
    }
  | {
      readonly type: 'text';
      readonly color: string;
      readonly fontSize: number;
      readonly text: string;
    };

export type WatermarkRenderOptions = {
  readonly angle: number;
  readonly horizontalGap: number;
  readonly mimeType: OutputImageType;
  readonly opacityPercent: number;
  readonly qualityPercent: number;
  readonly verticalGap: number;
};

export type WatermarkRenderResult = {
  readonly blob: Blob;
  readonly height: number;
  readonly mimeType: OutputImageType;
  readonly width: number;
};

export async function renderWatermark(
  sourceFile: File,
  content: WatermarkContent,
  options: WatermarkRenderOptions,
): Promise<WatermarkRenderResult> {
  const source = await loadImage(sourceFile);
  const dimensions = initialTargetDimensions(source.naturalWidth, source.naturalHeight);
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
  context.drawImage(source, 0, 0, canvas.width, canvas.height);

  context.save();
  context.translate(canvas.width / 2, canvas.height / 2);
  context.rotate(degreesToRadians(options.angle));
  context.globalAlpha = normalizePercentage(options.opacityPercent, 28) / 100;
  if (content.type === 'text') {
    drawTextWatermarks(context, canvas, content, options);
  } else {
    await drawImageWatermarks(context, canvas, content, options);
  }
  context.restore();

  const blob = await encodeCanvas(canvas, options.mimeType, options.qualityPercent);
  if (blob.type !== options.mimeType) throw new Error('errors.outputFormatUnavailable');
  return { blob, mimeType: options.mimeType, ...dimensions };
}

function drawTextWatermarks(
  context: CanvasRenderingContext2D,
  canvas: HTMLCanvasElement,
  content: Extract<WatermarkContent, { readonly type: 'text' }>,
  options: WatermarkRenderOptions,
): void {
  const fontSize = Math.max(8, content.fontSize);
  context.font = `600 ${String(fontSize)}px sans-serif`;
  context.fillStyle = content.color;
  context.textAlign = 'center';
  context.textBaseline = 'middle';
  const markWidth = Math.max(1, context.measureText(content.text).width);
  const markHeight = fontSize * 1.4;
  for (const { x, y } of createTileOrigins(
    canvas.width,
    canvas.height,
    markWidth,
    markHeight,
    options.horizontalGap,
    options.verticalGap,
  )) {
    context.fillText(content.text, x, y);
  }
}

async function drawImageWatermarks(
  context: CanvasRenderingContext2D,
  canvas: HTMLCanvasElement,
  content: Extract<WatermarkContent, { readonly type: 'image' }>,
  options: WatermarkRenderOptions,
): Promise<void> {
  const watermark = await loadImage(content.file);
  const markWidth = Math.max(16, content.width);
  const markHeight = Math.max(1, (markWidth * watermark.naturalHeight) / watermark.naturalWidth);
  for (const { x, y } of createTileOrigins(
    canvas.width,
    canvas.height,
    markWidth,
    markHeight,
    options.horizontalGap,
    options.verticalGap,
  )) {
    context.drawImage(watermark, x - markWidth / 2, y - markHeight / 2, markWidth, markHeight);
  }
}

async function loadImage(file: File): Promise<HTMLImageElement> {
  const url = URL.createObjectURL(file);
  try {
    const image = new Image();
    image.decoding = 'async';
    image.src = url;
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
    URL.revokeObjectURL(url);
  }
}

async function encodeCanvas(
  canvas: HTMLCanvasElement,
  mimeType: OutputImageType,
  qualityPercent: number,
): Promise<Blob> {
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (blob === null) {
          reject(new Error('errors.imageEncodeFailed'));
          return;
        }
        resolve(blob);
      },
      mimeType,
      encoderQuality(qualityPercent),
    );
  });
}
