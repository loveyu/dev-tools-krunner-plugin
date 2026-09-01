import XMLBuilder from 'fast-xml-builder';
import { XMLParser } from 'fast-xml-parser';
import { SyntaxValidator } from 'fast-xml-validator';
import JSON5 from 'json5';
import { gzip, ungzip } from 'pako';
import Papa from 'papaparse';
import qs from 'qs';
import { parse as parseToml } from 'smol-toml';
import { parse as parseYaml, stringify as stringifyYaml } from 'yaml';

import type { DataValue, FormatId, WebCodec } from './types';

const MAX_DEEP_DECODE_DEPTH = 5;
const FORBIDDEN_OBJECT_KEYS = new Set(['__proto__', 'constructor', 'prototype']);

export const WEB_CODECS: Readonly<Partial<Record<FormatId, WebCodec>>> = {
  json: {
    parse: parseJson,
    stringify: stringifyJson,
  },
  'json-deep': {
    parse: (text) => deepDecode(parseJson(text)),
    stringify: (value) => stringifyJson(deepDecode(value)),
  },
  'json-min': {
    stringify: stringifyJsonMin,
  },
  'js-object': {
    parse: (text) => normalizeValue(JSON5.parse<unknown>(text)),
    stringify: (value) => stringifyJsValue(value, 0),
  },
  yaml: {
    parse: (text) => normalizeValue(parseYaml(text, { maxAliasCount: 100 })),
    stringify: (value) => stringifyYaml(value, { indent: 2, lineWidth: 0 }),
  },
  xml: {
    parse: parseXml,
    stringify: stringifyXml,
  },
  csv: tableCodec(','),
  tsv: tableCodec('\t'),
  toml: {
    parse: (text) => normalizeValue(parseToml(text)),
  },
  ini: {
    parse: parseIni,
  },
  'query-rfc1738': queryCodec('RFC1738'),
  'query-rfc3986': queryCodec('RFC3986'),
  cookie: {
    parse: parseCookie,
    stringify: stringifyCookie,
  },
  'postman-bulk': {
    parse: parsePostmanBulk,
    stringify: stringifyPostmanBulk,
  },
  line: {
    parse: (text) => text.replaceAll('\r\n', '\n').split('\n'),
    stringify: stringifyLines,
  },
  plain: {
    parse: (text) => text,
    stringify: stringifyPlain,
  },
  uri: {
    parse: parseUri,
  },
  jwt: {
    parse: parseJwt,
  },
  base64: {
    parse: decodeBase64,
    stringify: (value) => encodeBase64(stringifyPlain(value)),
  },
  'base64-gzip': {
    parse: decodeBase64Gzip,
    stringify: (value) => encodeBase64Bytes(gzip(stringifyPlain(value))),
  },
  'url-encode': {
    parse: decodeUrl,
    stringify: (value) => encodeURIComponent(stringifyPlain(value)).replaceAll('%20', '+'),
  },
};

export function normalizeValue(value: unknown, seen = new WeakSet<object>()): DataValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') {
    return value;
  }
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new Error('convert.errors.nanOrInfinity');
    }
    return value;
  }
  if (typeof value === 'bigint') {
    return value.toString();
  }
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (typeof value !== 'object') {
    throw new Error('convert.errors.unsupportedDataType');
  }
  if (seen.has(value)) {
    throw new Error('convert.errors.circularReference');
  }
  seen.add(value);
  try {
    if (Array.isArray(value)) {
      return value.map((item) => normalizeValue(item, seen));
    }
    const output: Record<string, DataValue> = Object.create(null) as Record<string, DataValue>;
    for (const [key, item] of Object.entries(value)) {
      if (FORBIDDEN_OBJECT_KEYS.has(key)) {
        throw new Error('convert.errors.forbiddenObjectKey');
      }
      output[key] = normalizeValue(item, seen);
    }
    return output;
  } finally {
    seen.delete(value);
  }
}

function parseJson(text: string): DataValue {
  return normalizeValue(JSON.parse(text) as unknown);
}

function stringifyJson(value: DataValue): string {
  return JSON.stringify(value, null, 2);
}

function stringifyJsonMin(value: DataValue): string {
  return JSON.stringify(value);
}

function deepDecode(value: DataValue, depth = 0): DataValue {
  if (typeof value === 'string' && depth < MAX_DEEP_DECODE_DEPTH) {
    const candidate = value.trim();
    if (looksLikeJsonValue(candidate)) {
      try {
        return deepDecode(parseJson(candidate), depth + 1);
      } catch {
        return value;
      }
    }
    return value;
  }
  if (isDataArray(value)) {
    return value.map((item) => deepDecode(item, depth));
  }
  if (isDataObject(value)) {
    const output: Record<string, DataValue> = Object.create(null) as Record<string, DataValue>;
    for (const [key, item] of Object.entries(value)) {
      output[key] = deepDecode(item, depth);
    }
    return output;
  }
  return value;
}

function looksLikeJsonValue(text: string): boolean {
  return (
    /^[{["]/.test(text) ||
    /^(?:true|false|null)$/.test(text) ||
    /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/.test(text)
  );
}

function stringifyJsValue(value: DataValue, depth: number): string {
  if (typeof value === 'string') {
    return JSON.stringify(value);
  }
  if (value === null || typeof value === 'boolean' || typeof value === 'number') {
    return String(value);
  }
  const indentation = '  '.repeat(depth);
  const childIndentation = '  '.repeat(depth + 1);
  if (isDataArray(value)) {
    if (value.length === 0) {
      return '[]';
    }
    const items = value.map((item) => `${childIndentation}${stringifyJsValue(item, depth + 1)}`);
    return `[\n${items.join(',\n')}\n${indentation}]`;
  }
  const entries = Object.entries(value);
  if (entries.length === 0) {
    return '{}';
  }
  const items = entries.map(([key, item]) => {
    const renderedKey = /^[A-Za-z_$][\w$]*$/.test(key) ? key : JSON.stringify(key);
    return `${childIndentation}${renderedKey}: ${stringifyJsValue(item, depth + 1)}`;
  });
  return `{\n${items.join(',\n')}\n${indentation}}`;
}

function parseXml(text: string): DataValue {
  if (/<!DOCTYPE/i.test(text)) {
    throw new Error('convert.errors.xmlDoctypeForbidden');
  }
  try {
    SyntaxValidator.validate(text);
  } catch (error: unknown) {
    throw new Error('convert.errors.xmlParseFailed', { cause: error });
  }
  const parser = new XMLParser({
    ignoreAttributes: false,
    parseAttributeValue: true,
    parseTagValue: true,
    processEntities: false,
    trimValues: false,
  });
  return normalizeValue(parser.parse(text) as unknown);
}

function stringifyXml(value: DataValue): string {
  if (!isDataObject(value)) {
    throw new Error('convert.errors.xmlRootMustBeObject');
  }
  const builder = new XMLBuilder({
    format: true,
    ignoreAttributes: false,
    processEntities: false,
    suppressEmptyNode: false,
  });
  const output: unknown = builder.build(mutableValue(value));
  if (typeof output !== 'string') {
    throw new Error('convert.errors.xmlBuilderOutputMissing');
  }
  return output;
}

function tableCodec(delimiter: ',' | '\t'): WebCodec {
  return {
    parse: (text) => parseTable(text, delimiter),
    stringify: (value) => stringifyTable(value, delimiter),
  };
}

function parseTable(text: string, delimiter: ',' | '\t'): DataValue {
  const result = Papa.parse<Record<string, string>>(text, {
    delimiter,
    header: true,
    skipEmptyLines: 'greedy',
  });
  const error = result.errors[0];
  if (error !== undefined) {
    throw new Error('convert.errors.tableParseFailed', { cause: error });
  }
  return normalizeValue(result.data);
}

function stringifyTable(value: DataValue, delimiter: ',' | '\t'): string {
  let rows: unknown[];
  if (isDataArray(value)) {
    rows = value.map((item) => mutableValue(item));
  } else if (isDataObject(value)) {
    rows = [mutableValue(value)];
  } else {
    throw new Error('convert.errors.tableRequiresArrayOrObject');
  }
  return Papa.unparse(rows, {
    delimiter,
    escapeFormulae: true,
    newline: '\n',
  });
}

function parseIni(text: string): DataValue {
  const root: Record<string, DataValue> = Object.create(null) as Record<string, DataValue>;
  let current = root;
  for (const sourceLine of text.replaceAll('\r\n', '\n').split('\n')) {
    const line = sourceLine.trim();
    if (line === '' || line.startsWith(';') || line.startsWith('#')) {
      continue;
    }
    const section = /^\[([^\]]+)]$/.exec(line);
    if (section !== null) {
      const name = section[1]?.trim() ?? '';
      assertSafeKey(name);
      // 同名 section 再次出现时沿用既有对象（同 key 覆盖、新 key 追加），
      // 与主流宽松 INI 解析器一致，避免前一段内容被静默丢弃。
      // 顶层标量键与节名同名冲突时不沿用标量（无法继续挂键）。
      const existing = root[name];
      if (existing !== undefined && typeof existing === 'object' && existing !== null) {
        current = existing as Record<string, DataValue>;
        continue;
      }
      const next: Record<string, DataValue> = Object.create(null) as Record<string, DataValue>;
      root[name] = next;
      current = next;
      continue;
    }
    const separator = line.search(/[=:]/);
    if (separator <= 0) {
      throw new Error('convert.errors.iniMissingSeparator');
    }
    const key = line.slice(0, separator).trim();
    assertSafeKey(key);
    current[key] = parseIniScalar(line.slice(separator + 1).trim());
  }
  return root;
}

function parseIniScalar(value: string): DataValue {
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    return value.slice(1, -1);
  }
  if (/^(?:true|yes|on)$/i.test(value)) {
    return true;
  }
  if (/^(?:false|no|off)$/i.test(value)) {
    return false;
  }
  if (/^-?(?:0|[1-9]\d*)(?:\.\d+)?$/.test(value)) {
    const number = Number(value);
    if (Number.isSafeInteger(number) || !Number.isInteger(number)) {
      return number;
    }
  }
  return value;
}

function queryCodec(format: 'RFC1738' | 'RFC3986'): WebCodec {
  return {
    parse: (text) => normalizeValue(qs.parse(text, queryParseOptions())),
    stringify: (value): string => {
      if (!isDataObject(value) && !isDataArray(value)) {
        throw new Error('convert.errors.queryRequiresArrayOrObject');
      }
      return qs.stringify(mutableValue(value), {
        allowDots: false,
        arrayFormat: 'indices',
        encodeValuesOnly: false,
        format,
      });
    },
  };
}

function queryParseOptions(): qs.IParseOptions {
  return {
    allowDots: false,
    allowPrototypes: false,
    depth: 20,
    parameterLimit: 1000,
    strictNullHandling: true,
  };
}

function parseCookie(text: string): DataValue {
  const output: Record<string, DataValue> = Object.create(null) as Record<string, DataValue>;
  for (const pair of text.split(';')) {
    const trimmed = pair.trim();
    if (trimmed === '') {
      continue;
    }
    const separator = trimmed.indexOf('=');
    const rawKey = separator === -1 ? trimmed : trimmed.slice(0, separator);
    const rawValue = separator === -1 ? '' : trimmed.slice(separator + 1);
    const key = decodeURIComponent(rawKey.trim());
    assertSafeKey(key);
    output[key] = decodeURIComponent(rawValue);
  }
  return output;
}

function stringifyCookie(value: DataValue): string {
  if (!isDataObject(value)) {
    throw new Error('convert.errors.cookieRequiresObject');
  }
  return Object.entries(value)
    .map(
      ([key, item]) =>
        `${encodeURIComponent(key)}=${encodeURIComponent(stringifyCookieValue(item))}`,
    )
    .join('; ');
}

function stringifyCookieValue(value: DataValue): string {
  if (value === null) {
    return '';
  }
  return typeof value === 'object' ? stringifyJsonMin(value) : String(value);
}

function parsePostmanBulk(text: string): DataValue {
  const query = text
    .replaceAll('\r\n', '\n')
    .split('\n')
    .filter((line) => line !== '')
    .map((line) => {
      const separator = line.indexOf(':');
      const key = separator === -1 ? line : line.slice(0, separator);
      const value = separator === -1 ? '' : line.slice(separator + 1);
      return `${encodeURIComponent(key)}=${encodeURIComponent(value.replaceAll('↵', '\n'))}`;
    })
    .join('&');
  return normalizeValue(qs.parse(query, queryParseOptions()));
}

function stringifyPostmanBulk(value: DataValue): string {
  if (!isDataObject(value) && !isDataArray(value)) {
    throw new Error('convert.errors.postmanRequiresArrayOrObject');
  }
  const query = qs.stringify(mutableValue(value), {
    allowDots: false,
    arrayFormat: 'indices',
    encodeValuesOnly: false,
    format: 'RFC3986',
  });
  if (query === '') {
    return '';
  }
  return query.split('&').map(queryPairToPostmanLine).join('\n');
}

function queryPairToPostmanLine(pair: string): string {
  const separator = pair.indexOf('=');
  const key = separator === -1 ? pair : pair.slice(0, separator);
  const value = separator === -1 ? '' : pair.slice(separator + 1);
  return `${decodeURIComponent(key)}:${decodeURIComponent(value).replaceAll('\n', '↵')}`;
}

function stringifyLines(value: DataValue): string {
  const items = isDataArray(value) ? value : isDataObject(value) ? Object.values(value) : [value];
  return items
    .map((item) =>
      typeof item === 'object' && item !== null
        ? stringifyJsonMin(item)
        : item === null
          ? ''
          : String(item),
    )
    .join('\n');
}

function stringifyPlain(value: DataValue): string {
  return typeof value === 'string' ? value : stringifyJsonMin(value);
}

function parseUri(text: string): DataValue {
  const url = new URL(text);
  const output: Record<string, DataValue> = Object.create(null) as Record<string, DataValue>;
  output['scheme'] = url.protocol.slice(0, -1);
  output['host'] = url.hostname;
  if (url.port !== '') output['port'] = Number(url.port);
  if (url.username !== '') output['user'] = decodeURIComponent(url.username);
  if (url.password !== '') output['pass'] = decodeURIComponent(url.password);
  output['path'] = url.pathname;
  if (url.search !== '') {
    output['query'] = url.search.slice(1);
    output['queryObject'] = normalizeValue(qs.parse(url.search.slice(1), queryParseOptions()));
  }
  if (url.hash !== '') output['fragment'] = url.hash.slice(1);
  return output;
}

function parseJwt(text: string): DataValue {
  const parts = text.trim().split('.');
  if (
    parts.length !== 3 ||
    parts[0] === undefined ||
    parts[1] === undefined ||
    parts[2] === undefined
  ) {
    throw new Error('convert.errors.jwtSegments');
  }
  return normalizeValue({
    headers: JSON.parse(decodeBase64Url(parts[0])) as unknown,
    claims: JSON.parse(decodeBase64Url(parts[1])) as unknown,
    signature: parts[2],
  });
}

function encodeBase64(text: string): string {
  return encodeBase64Bytes(new TextEncoder().encode(text));
}

function encodeBase64Bytes(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function decodeBase64(text: string): string {
  const cleaned = text.replaceAll(/\s/g, '');
  if (cleaned === '' || !/^[A-Za-z0-9+/]*={0,2}$/.test(cleaned) || cleaned.length % 4 !== 0) {
    throw new Error('convert.errors.invalidBase64');
  }
  return decodeBase64Binary(cleaned);
}

function decodeBase64Url(text: string): string {
  const normalized = text.replaceAll('-', '+').replaceAll('_', '/');
  const padding = '='.repeat((4 - (normalized.length % 4)) % 4);
  return decodeBase64Binary(normalized + padding);
}

function decodeBase64Binary(text: string): string {
  const binary = atob(text);
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
}

function decodeBase64Gzip(text: string): string {
  const cleaned = text.replaceAll(/\s/g, '');
  if (cleaned === '' || !/^[A-Za-z0-9+/]*={0,2}$/.test(cleaned)) {
    throw new Error('convert.errors.invalidBase64Gzip');
  }
  const binary = atob(cleaned);
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  return new TextDecoder('utf-8', { fatal: true }).decode(ungzip(bytes));
}

function decodeUrl(text: string): string {
  return decodeURIComponent(text.replaceAll('+', ' '));
}

function mutableValue(value: DataValue): unknown {
  if (isDataArray(value)) return value.map(mutableValue);
  if (isDataObject(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, mutableValue(item)]),
    );
  }
  return value;
}

function isDataArray(value: DataValue): value is readonly DataValue[] {
  return Array.isArray(value);
}

function isDataObject(value: DataValue): value is Readonly<Record<string, DataValue>> {
  return typeof value === 'object' && value !== null && !isDataArray(value);
}

function assertSafeKey(key: string): void {
  if (key === '' || FORBIDDEN_OBJECT_KEYS.has(key)) {
    throw new Error('convert.errors.forbiddenKey');
  }
}
