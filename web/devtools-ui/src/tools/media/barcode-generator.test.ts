import { describe, expect, it } from 'vitest';

import { buildBarcodeOptions } from './barcode-generator';
import type { BarcodeGenerationRequest } from './types';

function request(overrides: Partial<BarcodeGenerationRequest> = {}): BarcodeGenerationRequest {
  return { format: 'qrcode', text: ' hello ', scale: 4, ...overrides };
}

describe('barcode generator options', () => {
  it('builds QR and linear barcode options', () => {
    expect(buildBarcodeOptions(request())).toMatchObject({
      bcid: 'qrcode',
      text: ' hello ',
      scale: 4,
      includetext: false,
    });
    expect(buildBarcodeOptions(request({ format: 'code128' }))).toMatchObject({
      bcid: 'code128',
      height: 12,
      includetext: true,
    });
    expect(buildBarcodeOptions(request({ format: 'ean13', text: '123456789012' }))).toMatchObject({
      bcid: 'ean13',
    });
    expect(buildBarcodeOptions(request({ format: 'ean13', text: '1234567890128' }))).toMatchObject({
      bcid: 'ean13',
    });
  });

  it('rejects empty content', () => {
    expect(() => buildBarcodeOptions(request({ text: '  ' }))).toThrow('ui.enterContentToEncode');
  });

  it.each([0, 9, 1.5])('rejects invalid scale %s', (scale) => {
    expect(() => buildBarcodeOptions(request({ scale }))).toThrow(
      'ui.scaleMustBeAnIntegerFrom1To8',
    );
  });

  it('rejects invalid EAN-13 data', () => {
    expect(() => buildBarcodeOptions(request({ format: 'ean13', text: 'abc' }))).toThrow(
      'ui.ean13Requires12DigitsOr13DigitsIncludingThe',
    );
  });
});
