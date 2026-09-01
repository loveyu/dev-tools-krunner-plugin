import { describe, expect, it } from 'vitest';

import {
  bytesToBase64,
  firstImageFile,
  MAX_IMAGE_BYTES,
  prepareImage,
  validateImageMetadata,
} from './image';

describe('media image preparation', () => {
  it('validates supported image metadata and size limits', () => {
    expect(validateImageMetadata('text/plain', 1)).toContain(
      'ui.onlyPngJpegBmpTiffWebpAndGifImagesAre',
    );
    expect(validateImageMetadata('image/png', 0)).toContain('ui.theImageIsEmpty');
    expect(validateImageMetadata('image/png', MAX_IMAGE_BYTES + 1)).toContain(
      'ui.theImageMustNotExceed10Mib',
    );
    expect(validateImageMetadata('image/jpeg', 20)).toBeNull();
  });

  it('encodes empty, small and multi-chunk byte arrays', () => {
    expect(bytesToBase64(new Uint8Array())).toBe('');
    expect(bytesToBase64(new Uint8Array([0, 1, 2, 253, 254, 255]))).toBe('AAEC/f7/');
    const large = new Uint8Array(0x8001).fill(65);
    expect(atob(bytesToBase64(large))).toHaveLength(0x8001);
  });

  it('prepares a valid browser file', async () => {
    const file = new File([new Uint8Array([1, 2, 3])], 'sample.png', { type: 'image/png' });

    await expect(prepareImage(file)).resolves.toEqual({
      imageBase64: 'AQID',
      mimeType: 'image/png',
    });
  });

  it('rejects an invalid browser file', async () => {
    const file = new File(['text'], 'sample.txt', { type: 'text/plain' });

    await expect(prepareImage(file)).rejects.toThrow('ui.onlyPngJpegBmpTiffWebpAndGifImagesAre');
  });

  it('selects the first image file', () => {
    const text = new File(['text'], 'sample.txt', { type: 'text/plain' });
    const image = new File(['image'], 'sample.webp', { type: 'image/webp' });

    expect(firstImageFile([text, image])).toBe(image);
    expect(firstImageFile([text])).toBeNull();
    expect(firstImageFile([])).toBeNull();
  });
});
