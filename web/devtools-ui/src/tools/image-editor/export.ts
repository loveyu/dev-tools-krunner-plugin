export const EDITOR_EXPORT_FORMATS = ['png', 'jpeg'] as const;

export type EditorExportFormat = (typeof EDITOR_EXPORT_FORMATS)[number];

export function dataUrlToBlob(dataUrl: string): Blob {
  const match = /^data:(image\/(?:png|jpeg));base64,([a-z\d+/=]+)$/i.exec(dataUrl);
  if (match === null) throw new Error('ui.theEditorReturnedInvalidImageData');
  const [, mimeType, payload] = match as unknown as [string, string, string];
  const binary = atob(payload);
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  return new Blob([bytes], { type: mimeType.toLowerCase() });
}

export function normalizeExportQuality(qualityPercent: number): number {
  if (!Number.isFinite(qualityPercent)) return 0.92;
  return Math.min(100, Math.max(1, qualityPercent)) / 100;
}

export function editedImageFilename(inputName: string, format: EditorExportFormat): string {
  const lastDot = inputName.lastIndexOf('.');
  const baseName = lastDot > 0 ? inputName.slice(0, lastDot) : inputName;
  const extension = format === 'jpeg' ? 'jpg' : 'png';
  return `${baseName || 'image'}.edited.${extension}`;
}
