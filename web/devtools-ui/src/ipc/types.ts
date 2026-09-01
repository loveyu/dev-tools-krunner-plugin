import type {
  ConversionDirection,
  ConverterCapabilities,
  NativeFormatId,
} from '../tools/converter/types';
import type { MediaCapabilities, MediaOperation, OcrOptions } from '../tools/media/types';
import type { MetadataCapabilities, MetadataDocument } from '../tools/metadata/types';
import type { PickedColor } from '../tools/color/model';

export type ThemeMode = 'system' | 'light' | 'dark';
export type LanguageMode = 'system' | 'zh-CN' | 'zh-TW' | 'en-US';
export type MetadataBackend = 'builtin' | 'external';

export type Settings = {
  readonly showTray: boolean;
  readonly autostart: boolean;
  readonly globalShortcutEnabled: boolean;
  readonly globalShortcut: string;
  readonly quickInputEnabled: boolean;
  readonly quickInputShortcut: string;
  readonly quickInputWidth: number;
  readonly quickInputHeight: number;
  readonly theme: ThemeMode;
  readonly language: LanguageMode;
  readonly metadataBackend: MetadataBackend;
};

export type InitialState = {
  readonly version: string;
  readonly settings: Settings;
  readonly converterCapabilities: ConverterCapabilities;
  readonly mediaCapabilities: MediaCapabilities;
  readonly metadataCapabilities: MetadataCapabilities;
};

export type WebRequest =
  | { readonly type: 'frontendReady' }
  | { readonly type: 'clipboardWrite'; readonly text: string }
  | {
      readonly type: 'nativeConvert';
      readonly requestId: string;
      readonly format: NativeFormatId;
      readonly direction: ConversionDirection;
      readonly payload: string;
    }
  | { readonly type: 'metadataPick'; readonly requestId: string }
  | {
      readonly type: 'metadataImage';
      readonly requestId: string;
      readonly imageBase64: string;
      readonly mimeType: string;
    }
  | { readonly type: 'colorPick'; readonly requestId: string }
  | {
      readonly type: 'mediaProcess';
      readonly requestId: string;
      readonly operation: MediaOperation;
      readonly imageBase64: string;
      readonly mimeType: string;
      readonly options: OcrOptions | Record<string, never>;
    }
  | { readonly type: 'settingsGet' }
  | { readonly type: 'windowHide' }
  | { readonly type: 'settingsUpdate'; readonly settings: Settings };

export type OpenJsonDetail = {
  readonly payload: string;
};

export type OpenConvertDetail = {
  readonly payload: string;
};

export type NativeConvertResultDetail = {
  readonly requestId: string;
  readonly result: string | null;
  readonly error: string | null;
};

export type MediaProcessResultDetail = {
  readonly requestId: string;
  readonly result: object | null;
  readonly error: string | null;
};

export type SettingsDetail = {
  readonly settings: Settings;
  readonly error: string | null;
};

export type MetadataProcessResultDetail = {
  readonly requestId: string;
  readonly result: MetadataDocument | null;
  readonly error: string | null;
};

export type ColorPickResultDetail = {
  readonly requestId: string;
  readonly color: PickedColor | null;
  readonly cancelled: boolean;
  readonly error: string | null;
};
