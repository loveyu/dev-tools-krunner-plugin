import type { MetadataBackend } from '../../ipc/types';

export type MetadataCapabilities = {
  readonly builtinVersion: string;
  readonly externalAvailable: boolean;
  readonly externalVersion: string | null;
};

export type MetadataField = {
  readonly group: string;
  readonly name: string;
  readonly value: string;
};

export type MetadataDocument = {
  readonly fileName: string;
  readonly backend: MetadataBackend;
  readonly fields: readonly MetadataField[];
};
