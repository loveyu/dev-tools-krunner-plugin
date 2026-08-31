import type { NativeConversionRequest, NativeExecutor } from '../tools/converter/types';
import { postRequest } from './bridge';
import type { NativeConvertResultDetail } from './types';

type PendingRequest = {
  readonly resolve: (value: string) => void;
  readonly reject: (reason: Error) => void;
  readonly timeout: number;
};

const pending = new Map<string, PendingRequest>();
let nextRequestId = 0;

window.addEventListener('devtools:native-convert-result', handleNativeResult);

export const executeNativeConversion: NativeExecutor = (
  request: NativeConversionRequest,
): Promise<string> =>
  new Promise((resolve, reject) => {
    const requestId = `native-${String(nextRequestId)}`;
    nextRequestId += 1;
    const timeout = window.setTimeout(() => {
      pending.delete(requestId);
      reject(new Error('本地转换执行超时'));
    }, 10_000);
    pending.set(requestId, { resolve, reject, timeout });

    const posted = postRequest({
      type: 'nativeConvert',
      requestId,
      format: request.format,
      direction: request.direction,
      payload: request.payload,
    });
    if (!posted) {
      window.clearTimeout(timeout);
      pending.delete(requestId);
      reject(new Error('当前环境未提供本地转换 IPC'));
    }
  });

function handleNativeResult(event: Event): void {
  if (!(event instanceof CustomEvent) || !isNativeResult(event.detail)) return;
  const request = pending.get(event.detail.requestId);
  if (request === undefined) return;
  window.clearTimeout(request.timeout);
  pending.delete(event.detail.requestId);
  if (event.detail.error !== null) {
    request.reject(new Error(event.detail.error));
  } else if (event.detail.result !== null) {
    request.resolve(event.detail.result);
  } else {
    request.reject(new Error('本地转换未返回结果'));
  }
}

function isNativeResult(detail: unknown): detail is NativeConvertResultDetail {
  if (typeof detail !== 'object' || detail === null) return false;
  const candidate = detail as Record<string, unknown>;
  return (
    typeof candidate['requestId'] === 'string' &&
    (candidate['result'] === null || typeof candidate['result'] === 'string') &&
    (candidate['error'] === null || typeof candidate['error'] === 'string')
  );
}
