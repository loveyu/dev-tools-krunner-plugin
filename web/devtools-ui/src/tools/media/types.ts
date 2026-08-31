export type OcrCapability = {
  readonly available: boolean;
  readonly version: string | null;
  readonly languages: readonly string[];
};

export type BarcodeCapability = {
  readonly available: boolean;
  readonly version: string | null;
};

export type MediaCapabilities = {
  readonly ocr: OcrCapability;
  readonly barcode: BarcodeCapability;
};

export type PreparedImage = {
  readonly imageBase64: string;
  readonly mimeType: string;
};

export type MediaOperation = 'barcode' | 'ocr';

export type OcrOptions = {
  readonly language: string;
  readonly pageSegmentationMode: number;
  readonly minimumConfidence: number;
};

export type OcrWord = {
  readonly text: string;
  readonly confidence: number;
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
};

export type OcrResult = {
  readonly fullText: string;
  readonly averageConfidence: number;
  readonly words: readonly OcrWord[];
};

export type DetectedCode = {
  readonly codeType: string;
  readonly data: string;
};

export type BarcodeRecognitionResult = {
  readonly codes: readonly DetectedCode[];
};

export type MediaProcessingRequest =
  | ({ readonly operation: 'ocr'; readonly options: OcrOptions } & PreparedImage)
  | ({ readonly operation: 'barcode'; readonly options: Record<string, never> } & PreparedImage);

export type BarcodeFormat = 'code128' | 'code39' | 'ean13' | 'qrcode';

export type BarcodeGenerationRequest = {
  readonly format: BarcodeFormat;
  readonly text: string;
  readonly scale: number;
};
