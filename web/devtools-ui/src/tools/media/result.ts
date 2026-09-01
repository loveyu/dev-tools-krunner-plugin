import type { BarcodeRecognitionResult, OcrResult, OcrWord } from './types';

export function parseOcrResult(value: unknown): OcrResult {
  if (!isRecord(value) || !Array.isArray(value['words'])) {
    throw new Error('ui.ocrReturnedAnInvalidResult');
  }
  const fullText = value['fullText'];
  const averageConfidence = value['averageConfidence'];
  if (typeof fullText !== 'string' || typeof averageConfidence !== 'number') {
    throw new Error('ui.ocrReturnedAnInvalidResult');
  }
  const words = value['words'].map(parseOcrWord);
  return { fullText, averageConfidence, words };
}

export function parseBarcodeResult(value: unknown): BarcodeRecognitionResult {
  if (!isRecord(value) || !Array.isArray(value['codes'])) {
    throw new Error('ui.barcodeRecognitionReturnedAnInvalidResult');
  }
  const codes = value['codes'].map((item): { readonly codeType: string; readonly data: string } => {
    if (
      !isRecord(item) ||
      typeof item['codeType'] !== 'string' ||
      typeof item['data'] !== 'string'
    ) {
      throw new Error('ui.barcodeRecognitionReturnedAnInvalidResult');
    }
    return { codeType: item['codeType'], data: item['data'] };
  });
  return { codes };
}

function parseOcrWord(value: unknown): OcrWord {
  if (!isRecord(value)) throw new Error('ui.ocrReturnedAnInvalidTextBox');
  const text = value['text'];
  const confidence = value['confidence'];
  const left = value['left'];
  const top = value['top'];
  const width = value['width'];
  const height = value['height'];
  if (
    typeof text !== 'string' ||
    typeof confidence !== 'number' ||
    typeof left !== 'number' ||
    typeof top !== 'number' ||
    typeof width !== 'number' ||
    typeof height !== 'number'
  ) {
    throw new Error('ui.ocrReturnedAnInvalidTextBox');
  }
  return { text, confidence, left, top, width, height };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
