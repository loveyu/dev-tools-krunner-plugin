import type { WebRequest } from './types';

export function postRequest(request: WebRequest): boolean {
  if (window.ipc === undefined) {
    return false;
  }

  window.ipc.postMessage(JSON.stringify(request));
  return true;
}
