import { describe, expect, it } from 'vitest';

import { dataUrlToBlob, editedImageFilename, normalizeExportQuality } from './export';

describe('image editor export', () => {
  it('converts PNG and JPEG data URLs into blobs', async () => {
    const png = dataUrlToBlob('data:image/png;base64,aGVsbG8=');
    const jpeg = dataUrlToBlob('data:image/jpeg;base64,d29ybGQ=');

    expect(png.type).toBe('image/png');
    expect(await png.text()).toBe('hello');
    expect(jpeg.type).toBe('image/jpeg');
    expect(await jpeg.text()).toBe('world');
  });

  it('rejects invalid and incomplete image data URLs', () => {
    expect(() => dataUrlToBlob('https://example.com/image.png')).toThrow('无效');
    expect(() => dataUrlToBlob('data:image/png;base64,')).toThrow('无效');
  });

  it('normalizes JPEG quality', () => {
    expect(normalizeExportQuality(Number.NaN)).toBe(0.92);
    expect(normalizeExportQuality(0)).toBe(0.01);
    expect(normalizeExportQuality(80)).toBe(0.8);
    expect(normalizeExportQuality(120)).toBe(1);
  });

  it('builds PNG and JPEG filenames', () => {
    expect(editedImageFilename('photo.webp', 'png')).toBe('photo.edited.png');
    expect(editedImageFilename('archive.photo.png', 'jpeg')).toBe('archive.photo.edited.jpg');
    expect(editedImageFilename('.hidden', 'png')).toBe('.hidden.edited.png');
    expect(editedImageFilename('', 'png')).toBe('image.edited.png');
  });
});
