import CryptoJS from 'crypto-js';
import { describe, expect, it, vi } from 'vitest';

import {
  CIPHER_ALGORITHMS,
  decryptText,
  encryptText,
  isLegacyCipher,
  IT_TOOLS_PROJECT_URL,
} from './model';

describe('crypto workbench', () => {
  it.each(CIPHER_ALGORITHMS)('round-trips %s passphrase ciphertext', (algorithm) => {
    const encrypted = encryptText(algorithm, 'DevTools 你好', 'secret');
    expect(encrypted).not.toContain('DevTools');
    expect(decryptText(algorithm, encrypted, 'secret')).toBe('DevTools 你好');
  });

  it('rejects empty values and marks legacy ciphers', () => {
    expect(() => encryptText('AES', '', 'secret')).toThrow('Input must not be empty');
    expect(() => encryptText('AES', 'value', '')).toThrow('Passphrase must not be empty');
    expect(() => decryptText('AES', 'not-valid', 'secret')).toThrow(
      'Ciphertext must be OpenSSL salted Base64',
    );
    expect(isLegacyCipher('AES')).toBe(false);
    expect(isLegacyCipher('TripleDES')).toBe(true);
    expect(isLegacyCipher('Rabbit')).toBe(true);
    expect(isLegacyCipher('RC4')).toBe(true);
    expect(IT_TOOLS_PROJECT_URL).toBe('https://github.com/CorentinTh/it-tools');
  });

  it('preserves cipher failures as error causes', () => {
    const ciphertext = encryptText('AES', 'value', 'secret');
    vi.spyOn(CryptoJS.AES, 'decrypt').mockImplementationOnce(() => {
      throw new Error('broken cipher');
    });
    expect(() => decryptText('AES', ciphertext, 'secret')).toThrow('broken cipher');

    vi.spyOn(CryptoJS.AES, 'decrypt').mockImplementationOnce(() => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- 覆盖第三方库抛出非 Error 值的防御分支。
      throw 'broken value';
    });
    expect(() => decryptText('AES', ciphertext, 'secret')).toThrow('Unable to decrypt ciphertext');
  });
});
