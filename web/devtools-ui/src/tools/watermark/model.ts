import type { InjectionKey } from 'vue';

import type { OutputImageType } from '../image-compression/types';

export const WATERMARKER_PROJECT_URL = 'https://github.com/TransparentLC/watermarker';

/** 父视图向设置面板 provide 水印设置对象的注入键。 */
export const WATERMARK_SETTINGS_KEY: InjectionKey<WatermarkSettings> = Symbol('watermark-settings');

/** 原项目默认的时间占位符模板，插入光标处后可自由修改。 */
export const DEFAULT_TIME_TEMPLATE = '{Y}-{M}-{D} {h}:{m}:{s}';

/** 原图与水印图片共用同一张量上限，超限直接拒绝。 */
export const WATERMARK_IMAGE_TYPES = [
  'image/png',
  'image/jpeg',
  'image/webp',
  'image/bmp',
  'image/gif',
] as const;

export const WATERMARK_LIMITS = {
  opacity: { min: 0, max: 1, step: 0.01, default: 0.15 },
  angle: { min: -90, max: 90, step: 1, default: -45 },
  scale: { min: 0.1, max: 5, step: 0.05, default: 1 },
  fontSize: { min: 6, max: 256, step: 1, default: 24 },
  fontWeight: { min: 100, max: 900, step: 100, default: 400 },
  offset: { min: -1000, max: 1000, step: 1, default: 0 },
  gapX: { min: -1000, max: 2000, step: 1, default: 100 },
  gapY: { min: -1000, max: 2000, step: 1, default: 75 },
  quality: { min: 0, max: 1, step: 0.01, default: 0.9 },
} as const;

export type WatermarkMode = 'text' | 'image';

/** 设置对象是可变的响应式状态（视图与面板直接更新字段驱动实时预览），不加 readonly。 */
export type TextStyle = {
  fontSize: number;
  fontWeight: number;
  italic: boolean;
  outline: boolean;
  center: boolean;
  shadow: boolean;
  textColor: string;
  shadowColor: string;
};

export type WatermarkSettings = {
  mode: WatermarkMode;
  text: string;
  scale: number;
  opacity: number;
  angle: number;
  offsetX: number;
  offsetY: number;
  gapX: number;
  gapY: number;
  /** 仅图片模式：全图盖遮罩，只在水印图片处镂空透出原图。 */
  keepImageVisible: boolean;
  textStyle: TextStyle;
  outputType: OutputImageType;
  quality: number;
};

export type TileOrigin = {
  readonly x: number;
  readonly y: number;
};

export function degreesToRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

export function clampNumber(value: number, min: number, max: number, fallback: number): number {
  // 仅 NaN 回退默认值；±Infinity 参与钳制，得到边界值。
  if (Number.isNaN(value)) return fallback;
  return Math.min(max, Math.max(min, value));
}

/** 默认参数与原项目一致：0.15 不透明度、-45°、缩放 1、字号 24、字重 400。 */
export function createDefaultWatermarkSettings(text: string): WatermarkSettings {
  return {
    mode: 'text',
    text,
    scale: WATERMARK_LIMITS.scale.default,
    opacity: WATERMARK_LIMITS.opacity.default,
    angle: WATERMARK_LIMITS.angle.default,
    offsetX: WATERMARK_LIMITS.offset.default,
    offsetY: WATERMARK_LIMITS.offset.default,
    gapX: WATERMARK_LIMITS.gapX.default,
    gapY: WATERMARK_LIMITS.gapY.default,
    keepImageVisible: false,
    textStyle: {
      fontSize: WATERMARK_LIMITS.fontSize.default,
      fontWeight: WATERMARK_LIMITS.fontWeight.default,
      italic: false,
      outline: false,
      center: true,
      shadow: false,
      textColor: '#ffffff',
      shadowColor: '#000000',
    },
    outputType: 'image/png',
    quality: WATERMARK_LIMITS.quality.default,
  };
}

/** 时间占位符 token 集合；正则捕获结果即此联合，switch 可穷尽。 */
type TimeToken = 'Y' | 'M' | 'D' | 'h' | 'm' | 's';

/** 将 {Y}/{M}/{D}/{h}/{m}/{s} 占位符展开为当前时间，非占位符原样保留。 */
export function expandTimeTemplate(text: string, now: Date): string {
  return text.replace(/\{([YMDhms])\}/g, (_, token: TimeToken) => {
    switch (token) {
      case 'Y':
        return String(now.getFullYear()).padStart(4, '0');
      case 'M':
      case 'D':
      case 'h':
      case 'm':
      case 's':
        return String(timeComponent(token, now)).padStart(2, '0');
    }
  });
}

function timeComponent(token: Exclude<TimeToken, 'Y'>, now: Date): number {
  switch (token) {
    case 'M':
      return now.getMonth() + 1;
    case 'D':
      return now.getDate();
    case 'h':
      return now.getHours();
    case 'm':
      return now.getMinutes();
    case 's':
      return now.getSeconds();
  }
}

/** 按换行拆分多行水印；去除首尾空白行，中间空行保留为占位行，制表符展开为空格。 */
export function splitWatermarkLines(text: string): readonly string[] {
  const lines = text.replace(/\t/g, '    ').split(/\r?\n/);
  const first = lines.findIndex((line) => line.trim() !== '');
  if (first === -1) {
    return [];
  }
  const last = lines.findLastIndex((line) => line.trim() !== '');
  return lines.slice(first, last + 1);
}

/**
 * 计算平铺网格原点（画布中心坐标系）。旋转后需要覆盖的对角半径外加一个水印块。
 * 间隔允许为负（重叠平铺）；水印尺寸加间隔小于等于 1 px 时钳到 1 px，避免死循环。
 */
export function createTileOrigins(
  canvasWidth: number,
  canvasHeight: number,
  markWidth: number,
  markHeight: number,
  gapX: number,
  gapY: number,
  offsetX: number,
  offsetY: number,
): readonly TileOrigin[] {
  const values = [canvasWidth, canvasHeight, markWidth, markHeight];
  if (values.some((value) => !Number.isFinite(value) || value <= 0)) {
    throw new Error('watermark.errors.invalidDimensions');
  }
  if (![gapX, gapY, offsetX, offsetY].every((value) => Number.isFinite(value))) {
    throw new Error('watermark.errors.invalidLayout');
  }

  const radius =
    Math.ceil(Math.hypot(canvasWidth, canvasHeight) / 2) + Math.max(markWidth, markHeight);
  const stepX = Math.max(1, markWidth + gapX);
  const stepY = Math.max(1, markHeight + gapY);
  // 起点向左上取整到网格格点，保证 (offsetX, offsetY) 始终是网格中的一个原点。
  const startX = -Math.ceil(radius / stepX) * stepX + offsetX;
  const startY = -Math.ceil(radius / stepY) * stepY + offsetY;
  const origins: TileOrigin[] = [];
  for (let y = startY; y <= radius; y += stepY) {
    for (let x = startX; x <= radius; x += stepX) {
      origins.push({ x, y });
    }
  }
  return origins;
}

/** 预览渲染分辨率上限：导出始终使用原图分辨率，预览只限制最长边。 */
export const PREVIEW_MAX_DIMENSION = 1400;

export function previewScale(width: number, height: number): number {
  const longest = Math.max(width, height);
  return longest > PREVIEW_MAX_DIMENSION ? PREVIEW_MAX_DIMENSION / longest : 1;
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

/** 组装 canvas font 字符串：斜体、字重、字号顺序遵循 CSS font 简写。 */
export function canvasFontSpec(style: TextStyle, scaledFontSize: number): string {
  const italicPart = style.italic ? 'italic ' : '';
  return `${italicPart}${String(style.fontWeight)} ${String(Math.max(1, scaledFontSize))}px sans-serif`;
}
