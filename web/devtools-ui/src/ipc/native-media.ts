import { postRequest } from './bridge';
import type { MediaProcessResultDetail } from './types';
import { parseBarcodeResult, parseOcrResult } from '../tools/media/result';
import type {
  BarcodeRecognitionResult,
  MediaProcessingRequest,
  OcrResult,
} from '../tools/media/types';

type PendingRequest = {
  readonly resolve: (value: unknown) => void;
  readonly reject: (reason: Error) => void;
  readonly timeout: number;
};

const pending = new Map<string, PendingRequest>();
let nextRequestId = 0;

window.addEventListener('devtools:media-process-result', handleMediaResult);

export async function executeOcr(
  request: MediaProcessingRequest & { readonly operation: 'ocr' },
): Promise<OcrResult> {
  return parseOcrResult(await executeMediaProcessing(request));
}

export async function executeBarcode(
  request: MediaProcessingRequest & { readonly operation: 'barcode' },
): Promise<BarcodeRecognitionResult> {
  return parseBarcodeResult(await executeMediaProcessing(request));
}

function executeMediaProcessing(request: MediaProcessingRequest): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const requestId = `media-${String(nextRequestId)}`;
    nextRequestId += 1;
    const timeout = window.setTimeout(() => {
      pending.delete(requestId);
      reject(new Error('图片处理超时'));
    }, 45_000);
    pending.set(requestId, { resolve, reject, timeout });
    const posted = postRequest({
      type: 'mediaProcess',
      requestId,
      operation: request.operation,
      imageBase64: request.imageBase64,
      mimeType: request.mimeType,
      options: request.options,
    });
    if (!posted) {
      window.clearTimeout(timeout);
      pending.delete(requestId);
      reject(new Error('当前环境未提供图片处理 IPC'));
    }
  });
}

function handleMediaResult(event: Event): void {
  if (!(event instanceof CustomEvent) || !isMediaResult(event.detail)) return;
  const request = pending.get(event.detail.requestId);
  if (request === undefined) return;
  window.clearTimeout(request.timeout);
  pending.delete(event.detail.requestId);
  if (event.detail.error !== null) {
    request.reject(new Error(event.detail.error));
  } else if (event.detail.result !== null) {
    request.resolve(event.detail.result);
  } else {
    request.reject(new Error('图片处理未返回结果'));
  }
}

function isMediaResult(detail: unknown): detail is MediaProcessResultDetail {
  if (typeof detail !== 'object' || detail === null) return false;
  const candidate = detail as Record<string, unknown>;
  return (
    typeof candidate['requestId'] === 'string' &&
    (candidate['result'] === null || typeof candidate['result'] === 'object') &&
    (candidate['error'] === null || typeof candidate['error'] === 'string')
  );
}
