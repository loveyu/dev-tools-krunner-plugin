import { javascript } from '@codemirror/lang-javascript';
import { json } from '@codemirror/lang-json';
import { php } from '@codemirror/lang-php';
import { xml } from '@codemirror/lang-xml';
import { yaml } from '@codemirror/lang-yaml';
import { StreamLanguage } from '@codemirror/language';
import { properties } from '@codemirror/legacy-modes/mode/properties';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import type { Extension } from '@codemirror/state';

import type { FormatId } from '../converter/types';

/** 编辑器支持的语言标识；plain 表示无语法高亮的纯文本。 */
export type EditorLanguageId =
  'javascript' | 'json' | 'php' | 'plain' | 'properties' | 'toml' | 'xml' | 'yaml';

// StreamLanguage 与语言包扩展只构建一次，重复实例浪费内存且丢失内部缓存。
const propertiesLanguage = StreamLanguage.define(properties);
const tomlLanguage = StreamLanguage.define(toml);

const LANGUAGE_EXTENSIONS: Readonly<Record<EditorLanguageId, Extension | null>> = {
  javascript: javascript(),
  json: json(),
  php: php({ plain: true }),
  plain: null,
  properties: propertiesLanguage,
  toml: tomlLanguage,
  xml: xml(),
  yaml: yaml(),
};

export function languageExtension(language: EditorLanguageId): Extension | null {
  return LANGUAGE_EXTENSIONS[language] ?? null;
}

/**
 * 把数据转换格式映射到编辑器语言。
 *
 * 键值行格式（INI、Cookie、Postman Bulk）共用 properties 流模式；
 * 无法高亮的格式（Base64、JWT、CSV 等）回落为纯文本。
 */
export function languageOfFormat(format: FormatId): EditorLanguageId {
  switch (format) {
    case 'json':
    case 'json-deep':
    case 'json-min':
      return 'json';
    case 'js-object':
      return 'javascript';
    case 'yaml':
      return 'yaml';
    case 'xml':
      return 'xml';
    case 'toml':
      return 'toml';
    case 'cookie':
    case 'ini':
    case 'postman-bulk':
      return 'properties';
    case 'php-array':
    case 'php-var-export':
      return 'php';
    case 'base64':
    case 'base64-gzip':
    case 'csv':
    case 'jwt':
    case 'line':
    case 'php-serialize':
    case 'plain':
    case 'query-rfc1738':
    case 'query-rfc3986':
    case 'tsv':
    case 'uri':
    case 'url-encode':
      return 'plain';
  }
}
