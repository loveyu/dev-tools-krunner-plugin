import { postRequest } from './bridge';
import type { ColorPickResultDetail } from './types';
import type { PickedColor } from '../tools/color/model';

type PendingRequest = {
  readonly resolve: (value: PickedColor | null) => void;
  readonly reject: (reason: Error) => void;
  readonly timeout: number;
};

const pending = new Map<string, PendingRequest>();
let nextRequestId = 0;

window.addEventListener('devtools:color-pick-result', handleColorResult);

export function pickScreenColor(): Promise<PickedColor | null> {
  return new Promise((resolve, reject) => {
    const requestId = `color-${String(nextRequestId)}`;
    nextRequestId += 1;
    const timeout = window.setTimeout(() => {
      pending.delete(requestId);
      reject(new Error('ipc.errors.colorPickTimeout'));
    }, 120_000);
    pending.set(requestId, { resolve, reject, timeout });
    if (!postRequest({ type: 'colorPick', requestId })) {
      window.clearTimeout(timeout);
      pending.delete(requestId);
      reject(new Error('ipc.errors.colorPickUnavailable'));
    }
  });
}

function handleColorResult(event: Event): void {
  if (!(event instanceof CustomEvent) || !isColorResult(event.detail)) return;
  const request = pending.get(event.detail.requestId);
  if (request === undefined) return;
  window.clearTimeout(request.timeout);
  pending.delete(event.detail.requestId);
  if (event.detail.error !== null) request.reject(new Error(event.detail.error));
  else request.resolve(event.detail.cancelled ? null : event.detail.color);
}

function isColorResult(detail: unknown): detail is ColorPickResultDetail {
  if (typeof detail !== 'object' || detail === null) return false;
  const candidate = detail as Record<string, unknown>;
  return (
    typeof candidate['requestId'] === 'string' &&
    (candidate['color'] === null || typeof candidate['color'] === 'object') &&
    typeof candidate['cancelled'] === 'boolean' &&
    (candidate['error'] === null || typeof candidate['error'] === 'string')
  );
}
