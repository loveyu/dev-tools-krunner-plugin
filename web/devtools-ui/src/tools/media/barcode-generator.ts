import type { RenderOptions } from '@bwip-js/browser';

import type { BarcodeGenerationRequest } from './types';

export function buildBarcodeOptions(request: BarcodeGenerationRequest): RenderOptions {
  const text = request.text;
  if (text.trim() === '') throw new Error('请输入要编码的内容');
  if (!Number.isInteger(request.scale) || request.scale < 1 || request.scale > 8) {
    throw new Error('缩放比例必须是 1 到 8 的整数');
  }
  if (request.format === 'ean13' && !/^\d{12,13}$/u.test(text)) {
    throw new Error('EAN-13 需要输入 12 位数据或含校验位的 13 位数字');
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
