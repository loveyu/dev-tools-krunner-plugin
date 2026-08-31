import { SyntaxValidator } from 'fast-xml-validator';
import JSON5 from 'json5';
import { parse as parseToml } from 'smol-toml';
import { parse as parseYaml } from 'yaml';

import { WEB_CODECS } from './codecs';
import type { FormatId } from './types';

export function detectFormat(text: string, availableSources: ReadonlySet<FormatId>): FormatId {
  const value = text.trim();
  if (value === '') return 'plain';

  const candidates: readonly [FormatId, () => boolean][] = [
    ['jwt', (): boolean => isJwt(value)],
    ['json-deep', (): boolean => isJson(value)],
    ['xml', (): boolean => isXml(value)],
    ['uri', (): boolean => isUri(value)],
    ['php-serialize', (): boolean => isPhpSerialize(value)],
    ['js-object', (): boolean => isJsObject(value)],
    ['base64-gzip', (): boolean => isBase64Gzip(value)],
    ['cookie', (): boolean => isCookie(value)],
    ['query-rfc3986', (): boolean => isQuery(value)],
    ['tsv', (): boolean => isDelimitedTable(value, '\t')],
    ['csv', (): boolean => isDelimitedTable(value, ',')],
    ['postman-bulk', (): boolean => isPostmanBulk(value)],
    ['toml', (): boolean => isToml(value)],
    ['ini', (): boolean => isIni(value)],
    ['yaml', (): boolean => isYaml(value)],
    ['base64', (): boolean => isBase64(value)],
    ['url-encode', (): boolean => isUrlEncoded(value)],
  ];

  for (const [format, matches] of candidates) {
    if (availableSources.has(format) && matches()) return format;
  }
  return 'plain';
}

function isJson(text: string): boolean {
  if (!/^[{[]/.test(text)) return false;
  try {
    JSON.parse(text);
    return true;
  } catch {
    return false;
  }
}

function isJwt(text: string): boolean {
  if (!/^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*$/.test(text)) return false;
  try {
    WEB_CODECS.jwt?.parse?.(text);
    return true;
  } catch {
    return false;
  }
}

function isXml(text: string): boolean {
  if (!text.startsWith('<') || /<!DOCTYPE/i.test(text)) return false;
  try {
    SyntaxValidator.validate(text);
    return true;
  } catch {
    return false;
  }
}

function isUri(text: string): boolean {
  try {
    const url = new URL(text);
    return url.protocol.length > 1 && url.hostname !== '';
  } catch {
    return false;
  }
}

function isPhpSerialize(text: string): boolean {
  return /^(?:N;|[bisdaO]:)/.test(text);
}

function isJsObject(text: string): boolean {
  if (!/^[{[]/.test(text)) return false;
  try {
    JSON5.parse<unknown>(text);
    return true;
  } catch {
    return false;
  }
}

function isBase64Gzip(text: string): boolean {
  const compact = text.replaceAll(/\s/g, '');
  if (!isBase64(compact)) return false;
  try {
    const binary = atob(compact);
    return binary.charCodeAt(0) === 0x1f && binary.charCodeAt(1) === 0x8b;
  } catch {
    return false;
  }
}

function isCookie(text: string): boolean {
  return text.includes(';') && text.split(';').every((pair) => /^\s*[^=;\s]+\s*=/.test(pair));
}

function isQuery(text: string): boolean {
  return text.includes('=') && text.includes('&') && !text.includes('\n');
}

function isDelimitedTable(text: string, delimiter: ',' | '\t'): boolean {
  const lines = text.split(/\r?\n/).filter((line) => line !== '');
  if (lines.length < 2) return false;
  const columns = lines[0]?.split(delimiter).length ?? 0;
  return columns >= 2 && lines.every((line) => line.split(delimiter).length === columns);
}

function isPostmanBulk(text: string): boolean {
  const lines = text.split(/\r?\n/).filter((line) => line !== '');
  return lines.length >= 2 && lines.every((line) => /^[^:]+:/.test(line));
}

function isToml(text: string): boolean {
  if (!/^\s*(?:\[[^\]]+]|[A-Za-z0-9_.-]+\s*=)/m.test(text)) return false;
  if (
    !text.includes('[') &&
    !text.includes(']') &&
    !/=(?:\s*\[|\s*\{|\s*\d{4}-\d{2}-\d{2})/m.test(text)
  )
    return false;
  try {
    parseToml(text);
    return true;
  } catch {
    return false;
  }
}

function isIni(text: string): boolean {
  if (!/^\s*(?:\[[^\]]+]|[^=\s]+\s*=\s*\S)/m.test(text)) return false;
  try {
    WEB_CODECS.ini?.parse?.(text);
    return true;
  } catch {
    return false;
  }
}

function isYaml(text: string): boolean {
  if (!/^(?:---\s*$|\s*[-\w]+\s*:\s+)/m.test(text)) return false;
  try {
    const value: unknown = parseYaml(text, { maxAliasCount: 100 });
    return typeof value === 'object' && value !== null;
  } catch {
    return false;
  }
}

function isBase64(text: string): boolean {
  const compact = text.replaceAll(/\s/g, '');
  if (compact.length < 8 || compact.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(compact)) {
    return false;
  }
  try {
    WEB_CODECS.base64?.parse?.(compact);
    return true;
  } catch {
    return false;
  }
}

function isUrlEncoded(text: string): boolean {
  if (!/%[0-9A-Fa-f]{2}/.test(text)) return false;
  try {
    return decodeURIComponent(text) !== text;
  } catch {
    return false;
  }
}
