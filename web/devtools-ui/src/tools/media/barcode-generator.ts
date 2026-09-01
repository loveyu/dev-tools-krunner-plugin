import type { RenderOptions } from '@bwip-js/browser';

import type { BarcodeGenerationRequest } from './types';

export function buildBarcodeOptions(request: BarcodeGenerationRequest): RenderOptions {
  const text = request.text;
  if (text.trim() === '') throw new Error('ui.enterContentToEncode');
  if (!Number.isInteger(request.scale) || request.scale < 1 || request.scale > 8) {
    throw new Error('ui.scaleMustBeAnIntegerFrom1To8');
  }
  if (request.format === 'ean13' && !/^\d{12,13}$/u.test(text)) {
    throw new Error('ui.ean13Requires12DigitsOr13DigitsIncludingThe');
  }
  const common: RenderOptions = {
    bcid: request.format,
    text,
    scale: request.scale,
    includetext: request.format !== 'qrcode',
    textxalign: 'center',
    padding: 12,
    backgroundcolor: 'FFFFFF',
  };
  return request.format === 'qrcode' ? common : { ...common, height: 12 };
}
