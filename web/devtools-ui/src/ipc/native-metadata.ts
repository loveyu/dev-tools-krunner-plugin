import { postRequest } from './bridge';
import type { MetadataProcessResultDetail } from './types';
import type { MetadataDocument } from '../tools/metadata/types';
import type { PreparedImage } from '../tools/media/types';

type PendingRequest = {
  readonly resolve: (value: MetadataDocument) => void;
  readonly reject: (reason: Error) => void;
  readonly timeout: number;
};

const pending = new Map<string, PendingRequest>();
let nextRequestId = 0;

window.addEventListener('devtools:metadata-process-result', handleMetadataResult);

export function pickAndReadMetadata(): Promise<MetadataDocument> {
  return executeMetadataRequest((requestId) => ({ type: 'metadataPick', requestId }));
}

export function readImageMetadata(image: PreparedImage): Promise<MetadataDocument> {
  return executeMetadataRequest((requestId) => ({
    type: 'metadataImage',
    requestId,
    imageBase64: image.imageBase64,
    mimeType: image.mimeType,
  }));
}

function executeMetadataRequest(
  createRequest: (requestId: string) =>
    | { readonly type: 'metadataPick'; readonly requestId: string }
    | {
        readonly type: 'metadataImage';
        readonly requestId: string;
        readonly imageBase64: string;
        readonly mimeType: string;
      },
): Promise<MetadataDocument> {
  return new Promise((resolve, reject) => {
    const requestId = `metadata-${String(nextRequestId)}`;
    nextRequestId += 1;
    const timeout = window.setTimeout(() => {
      pending.delete(requestId);
      reject(new Error('ipc.errors.metadataTimeout'));
    }, 45_000);
    pending.set(requestId, { resolve, reject, timeout });
    if (!postRequest(createRequest(requestId))) {
      window.clearTimeout(timeout);
      pending.delete(requestId);
      reject(new Error('ipc.errors.metadataUnavailable'));
    }
  });
}

function handleMetadataResult(event: Event): void {
  if (!(event instanceof CustomEvent) || !isMetadataResult(event.detail)) return;
  const request = pending.get(event.detail.requestId);
  if (request === undefined) return;
  window.clearTimeout(request.timeout);
  pending.delete(event.detail.requestId);
  if (event.detail.error !== null) request.reject(new Error(event.detail.error));
  else if (event.detail.result !== null) request.resolve(event.detail.result);
  else request.reject(new Error('ipc.errors.metadataEmptyResult'));
}

function isMetadataResult(detail: unknown): detail is MetadataProcessResultDetail {
  if (typeof detail !== 'object' || detail === null) return false;
  const candidate = detail as Record<string, unknown>;
  return (
    typeof candidate['requestId'] === 'string' &&
    (candidate['result'] === null || typeof candidate['result'] === 'object') &&
    (candidate['error'] === null || typeof candidate['error'] === 'string')
  );
}
