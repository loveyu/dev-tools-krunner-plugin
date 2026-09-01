import CryptoJS from 'crypto-js';

export type CipherAlgorithm = 'AES' | 'Rabbit' | 'RC4' | 'TripleDES';

export const CIPHER_ALGORITHMS: readonly CipherAlgorithm[] = ['AES', 'TripleDES', 'Rabbit', 'RC4'];

export const IT_TOOLS_PROJECT_URL = 'https://github.com/CorentinTh/it-tools';

export function encryptText(
  algorithm: CipherAlgorithm,
  plaintext: string,
  passphrase: string,
): string {
  validateInputs(plaintext, passphrase);
  return cipherFor(algorithm).encrypt(plaintext, passphrase).toString();
}

export function decryptText(
  algorithm: CipherAlgorithm,
  ciphertext: string,
  passphrase: string,
): string {
  validateInputs(ciphertext, passphrase);
  const normalizedCiphertext = ciphertext.trim();
  if (!normalizedCiphertext.startsWith('U2FsdGVkX1')) {
    throw new Error('Ciphertext must be OpenSSL salted Base64');
  }
  try {
    return cipherFor(algorithm)
      .decrypt(normalizedCiphertext, passphrase)
      .toString(CryptoJS.enc.Utf8);
  } catch (error) {
    throw new Error(error instanceof Error ? error.message : 'Unable to decrypt ciphertext', {
      cause: error,
    });
  }
}

export function isLegacyCipher(algorithm: CipherAlgorithm): boolean {
  return algorithm !== 'AES';
}

type Cipher = {
  encrypt(message: string, passphrase: string): CryptoJS.lib.CipherParams;
  decrypt(ciphertext: string, passphrase: string): CryptoJS.lib.WordArray;
};

function cipherFor(algorithm: CipherAlgorithm): Cipher {
  switch (algorithm) {
    case 'AES':
      return CryptoJS.AES;
    case 'TripleDES':
      return CryptoJS.TripleDES;
    case 'Rabbit':
      return CryptoJS.Rabbit;
    case 'RC4':
      return CryptoJS.RC4;
  }
}

function validateInputs(value: string, passphrase: string): void {
  if (value === '') throw new Error('Input must not be empty');
  if (passphrase === '') throw new Error('Passphrase must not be empty');
}
