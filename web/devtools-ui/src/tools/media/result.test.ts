import { describe, expect, it } from 'vitest';

import { parseBarcodeResult, parseOcrResult } from './result';

const validWord = {
  text: 'Hello',
  confidence: 95,
  left: 1,
  top: 2,
  width: 3,
  height: 4,
};

describe('native media result validation', () => {
  it('parses valid OCR and barcode results', () => {
    expect(
      parseOcrResult({ fullText: 'Hello', averageConfidence: 95, words: [validWord] }),
    ).toEqual({ fullText: 'Hello', averageConfidence: 95, words: [validWord] });
    expect(parseBarcodeResult({ codes: [{ codeType: 'QR-Code', data: 'hello' }] })).toEqual({
      codes: [{ codeType: 'QR-Code', data: 'hello' }],
    });
  });

  it.each([null, {}, { words: null }])('rejects invalid OCR envelope %#', (value) => {
    expect(() => parseOcrResult(value)).toThrow('ui.ocrReturnedAnInvalidResult');
  });

  it.each([
    { fullText: null, averageConfidence: 1, words: [] },
    { fullText: '', averageConfidence: null, words: [] },
  ])('rejects invalid OCR summary %#', (value) => {
    expect(() => parseOcrResult(value)).toThrow('ui.ocrReturnedAnInvalidResult');
  });

  it.each([
    null,
    { ...validWord, text: null },
    { ...validWord, confidence: null },
    { ...validWord, left: null },
    { ...validWord, top: null },
    { ...validWord, width: null },
    { ...validWord, height: null },
  ])('rejects invalid OCR word %#', (word) => {
    expect(() => parseOcrResult({ fullText: '', averageConfidence: 0, words: [word] })).toThrow(
      'ui.ocrReturnedAnInvalidTextBox',
    );
  });

  it.each([null, {}, { codes: null }])('rejects invalid barcode envelope %#', (value) => {
    expect(() => parseBarcodeResult(value)).toThrow('ui.barcodeRecognitionReturnedAnInvalidResult');
  });

  it.each([null, { codeType: null, data: '' }, { codeType: 'QR-Code', data: null }])(
    'rejects invalid barcode item %#',
    (item) => {
      expect(() => parseBarcodeResult({ codes: [item] })).toThrow(
        'ui.barcodeRecognitionReturnedAnInvalidResult',
      );
    },
  );
});
