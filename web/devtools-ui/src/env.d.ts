/// <reference types="vite/client" />

import type { InitialState } from './ipc/types';

type PendingEvent = {
  readonly name: string;
  readonly detail: unknown;
};

declare global {
  interface Window {
    ipc?: {
      postMessage(message: string): void;
    };
    __DEVTOOLS_INITIAL_STATE__?: InitialState;
    __DEVTOOLS_PENDING_EVENTS__?: PendingEvent[];
    __DEVTOOLS_DISPATCH__?(name: string, detail: unknown): void;
  }
}

export {};
