import type { OutputImageType } from '../image-compression/types';
import {
  canvasFontSpec,
  createTileOrigins,
  degreesToRadians,
  expandTimeTemplate,
  previewScale,
  splitWatermarkLines,
} from './model';
import type { TextStyle, WatermarkSettings } from './model';

export type WatermarkRenderResult = {
  readonly blob: Blob;
  readonly height: number;
  readonly mimeType: OutputImageType;
  readonly width: number;
};

export type WatermarkRenderInput = {
  readonly source: HTMLImageElement;
  readonly watermarkImage: HTMLImageElement | null;
  readonly settings: WatermarkSettings;
  readonly now: Date;
  /** 预览传入分辨率上限；导出传 null 表示使用原图分辨率。 */
  readonly preview: boolean;
};

/** 遮罩镂空模式的黑纱不透明度，与水印不透明度滑块相互独立。 */
const KEEP_VISIBLE_MASK_ALPHA = 0.5;
const TEXT_LINE_HEIGHT_RATIO = 1.2;
const TEXT_OUTLINE_WIDTH_RATIO = 0.08;
const TEXT_SHADOW_BLUR_RATIO = 0.25;
/** 预览时间戳每秒刷新即可，模板展开精度为秒。 */
export const PREVIEW_CLOCK_TICK_MS = 1000;

/** 同步绘制水印并返回画布；预览直接展示画布，导出才需要编码成 blob。 */
export function composeWatermark(input: WatermarkRenderInput): HTMLCanvasElement {
  const { source, watermarkImage, settings, now, preview } = input;
  const scale = preview ? previewScale(source.naturalWidth, source.naturalHeight) : 1;
  const width = Math.max(1, Math.round(source.naturalWidth * scale));
  const height = Math.max(1, Math.round(source.naturalHeight * scale));

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext('2d');
  if (context === null) throw new Error('errors.canvasUnavailable');

  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = 'high';
  if (settings.outputType === 'image/jpeg') {
    // JPEG 无透明通道，透明原图直接编码会得到黑底，先垫白。
    context.fillStyle = '#ffffff';
    context.fillRect(0, 0, width, height);
  }
  context.drawImage(source, 0, 0, width, height);

  if (settings.mode === 'text') {
    drawTextWatermarks(context, width, height, settings, now, scale);
  } else if (watermarkImage !== null) {
    drawImageWatermarks(context, width, height, watermarkImage, settings, scale);
  }
  return canvas;
}

export async function renderWatermark(input: WatermarkRenderInput): Promise<WatermarkRenderResult> {
  const { settings } = input;
  const canvas = composeWatermark(input);
  const blob = await encodeCanvas(canvas, settings.outputType, settings.quality);
  if (blob.type !== settings.outputType) {
    throw new Error('watermark.errors.outputFormatUnavailable');
  }
  return { blob, mimeType: settings.outputType, width: canvas.width, height: canvas.height };
}

function drawTextWatermarks(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  settings: WatermarkSettings,
  now: Date,
  scale: number,
): void {
  const expanded = expandTimeTemplate(settings.text, now);
  const lines = splitWatermarkLines(expanded);
  if (lines.length === 0) return;

  const block = buildTextBlock(lines, settings.textStyle, scale * settings.scale);
  const origins = createTileOrigins(
    width,
    height,
    block.width,
    block.height,
    settings.gapX * scale,
    settings.gapY * scale,
    settings.offsetX * scale,
    settings.offsetY * scale,
  );

  context.save();
  // offset 已并入平铺原点网格，这里只负责把坐标系挪到画布中心并旋转。
  context.translate(width / 2, height / 2);
  context.rotate(degreesToRadians(settings.angle));
  context.globalAlpha = settings.opacity;
  for (const { x, y } of origins) {
    context.drawImage(block.canvas, x - block.width / 2, y - block.height / 2);
  }
  context.restore();
}

/** 水印块离屏画布：平铺时按 (x - width/2, y - height/2) 以中心定位。 */
type TextBlock = {
  readonly canvas: HTMLCanvasElement;
  readonly width: number;
  readonly height: number;
};

function buildTextBlock(lines: readonly string[], style: TextStyle, scale: number): TextBlock {
  const fontSize = Math.max(1, style.fontSize * scale);
  const lineHeight = fontSize * TEXT_LINE_HEIGHT_RATIO;
  const measure = document.createElement('canvas').getContext('2d');
  const font = canvasFontSpec(style, fontSize);
  if (measure !== null) {
    measure.font = font;
  }
  const lineWidths = lines.map((line) =>
    measure === null ? fontSize * 0.6 * line.length : measure.measureText(line).width,
  );
  const textWidth = Math.max(1, ...lineWidths);
  const canvas = document.createElement('canvas');
  canvas.width = Math.ceil(textWidth + fontSize * TEXT_SHADOW_BLUR_RATIO * 2);
  canvas.height = Math.ceil(lineHeight * lines.length + fontSize * TEXT_SHADOW_BLUR_RATIO);
  const context = canvas.getContext('2d');
  if (context === null) throw new Error('errors.canvasUnavailable');

  context.font = font;
  context.textBaseline = 'middle';
  context.textAlign = style.center ? 'center' : 'left';
  if (style.shadow) {
    context.shadowColor = style.shadowColor;
    context.shadowBlur = fontSize * TEXT_SHADOW_BLUR_RATIO;
  }
  const strokeWidth = Math.max(1, fontSize * TEXT_OUTLINE_WIDTH_RATIO);
  lines.forEach((line, index) => {
    const y = lineHeight * index + lineHeight / 2 + fontSize * TEXT_SHADOW_BLUR_RATIO;
    const x = style.center ? canvas.width / 2 : fontSize * TEXT_SHADOW_BLUR_RATIO;
    if (style.outline) {
      context.lineWidth = strokeWidth;
      context.strokeStyle = style.textColor;
      context.strokeText(line, x, y);
    } else {
      context.fillStyle = style.textColor;
      context.fillText(line, x, y);
    }
  });
  return { canvas, width: canvas.width, height: canvas.height };
}

function drawImageWatermarks(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  watermarkImage: HTMLImageElement,
  settings: WatermarkSettings,
  scale: number,
): void {
  if (watermarkImage.naturalWidth <= 0 || watermarkImage.naturalHeight <= 0) return;
  const markWidth = Math.max(1, watermarkImage.naturalWidth * settings.scale * scale);
  const markHeight = Math.max(
    1,
    (markWidth * watermarkImage.naturalHeight) / watermarkImage.naturalWidth,
  );
  const origins = createTileOrigins(
    width,
    height,
    markWidth,
    markHeight,
    settings.gapX * scale,
    settings.gapY * scale,
    settings.offsetX * scale,
    settings.offsetY * scale,
  );

  if (settings.keepImageVisible) {
    drawMaskedWatermarks(
      context,
      width,
      height,
      watermarkImage,
      origins,
      settings.angle,
      markWidth,
      markHeight,
    );
    return;
  }

  context.save();
  // offset 已并入平铺原点网格，这里只负责把坐标系挪到画布中心并旋转。
  context.translate(width / 2, height / 2);
  context.rotate(degreesToRadians(settings.angle));
  context.globalAlpha = settings.opacity;
  for (const { x, y } of origins) {
    context.drawImage(watermarkImage, x - markWidth / 2, y - markHeight / 2, markWidth, markHeight);
  }
  context.restore();
}

/** 「水印图片不遮原图」：黑纱盖全图，再在旋转平铺位置挖洞透出原图。 */
function drawMaskedWatermarks(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  watermarkImage: HTMLImageElement,
  origins: readonly { readonly x: number; readonly y: number }[],
  angle: number,
  markWidth: number,
  markHeight: number,
): void {
  const mask = document.createElement('canvas');
  mask.width = width;
  mask.height = height;
  const maskContext = mask.getContext('2d');
  if (maskContext === null) throw new Error('errors.canvasUnavailable');

  maskContext.fillStyle = `rgba(0, 0, 0, ${String(KEEP_VISIBLE_MASK_ALPHA)})`;
  maskContext.fillRect(0, 0, width, height);
  // 洞的位置与普通平铺一致：中心平移 + 旋转，offset 已并入原点网格。
  maskContext.translate(width / 2, height / 2);
  maskContext.rotate(degreesToRadians(angle));
  maskContext.globalCompositeOperation = 'destination-out';
  for (const { x, y } of origins) {
    maskContext.drawImage(
      watermarkImage,
      x - markWidth / 2,
      y - markHeight / 2,
      markWidth,
      markHeight,
    );
  }
  context.drawImage(mask, 0, 0);
}

export async function loadImageElement(file: File): Promise<HTMLImageElement> {
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
  quality: number,
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
      mimeType === 'image/png' ? undefined : quality,
    );
  });
}

/** 探测当前 WebView 实际支持的编码格式；WebKitGTK 可能缺少 WebP 编码。 */
export async function probeOutputSupport(
  types: readonly OutputImageType[],
): Promise<OutputImageType[]> {
  const canvas = document.createElement('canvas');
  canvas.width = 1;
  canvas.height = 1;
  const supported: OutputImageType[] = [];
  for (const type of types) {
    const blob = await encodeCanvas(canvas, type, 0.9);
    if (blob.type === type) {
      supported.push(type);
    }
  }
  return supported;
}
