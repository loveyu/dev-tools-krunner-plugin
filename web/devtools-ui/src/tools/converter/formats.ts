import type { FormatDefinition, FormatId } from './types';

export const FORMAT_DEFINITIONS: readonly FormatDefinition[] = [
  format('json', 'JSON', true, true),
  format('json-deep', 'JSON 深度解码', true, true),
  format('json-min', 'JSON 压缩', false, true),
  format('js-object', 'JS Object', true, true),
  format('yaml', 'YAML', true, true),
  format('xml', 'XML', true, true),
  format('csv', 'CSV', true, true),
  format('tsv', 'TSV', true, true),
  format('toml', 'TOML', true, false),
  format('ini', 'INI', true, false),
  format('query-rfc1738', 'Query String RFC1738', true, true),
  format('query-rfc3986', 'Query String RFC3986', true, true),
  format('cookie', 'Cookie', true, true),
  format('postman-bulk', 'Postman Bulk', true, true),
  format('line', 'Raw Line', true, true),
  format('plain', 'Plain Text', true, true),
  format('uri', 'URI', true, false),
  format('jwt', 'JWT（仅解码）', true, false),
  format('base64', 'Base64', true, true),
  format('base64-gzip', 'Base64 + Gzip', true, true),
  format('url-encode', 'URL Encode', true, true),
  nativeFormat('php-serialize', 'PHP Serialize', true, true),
  nativeFormat('php-var-export', 'PHP VarExport', false, true),
  nativeFormat('php-array', 'PHP Array', false, true),
] as const;

export function definitionOf(id: FormatId): FormatDefinition {
  const definition = FORMAT_DEFINITIONS.find((candidate) => candidate.id === id);
  if (definition === undefined) {
    throw new Error(`未知转换格式：${id}`);
  }
  return definition;
}

function format(
  id: FormatId,
  label: string,
  canParse: boolean,
  canStringify: boolean,
): FormatDefinition {
  return { id, label, canParse, canStringify, runtime: 'web' };
}

function nativeFormat(
  id: FormatId,
  label: string,
  canParse: boolean,
  canStringify: boolean,
): FormatDefinition {
  return { id, label, canParse, canStringify, runtime: 'native' };
}
