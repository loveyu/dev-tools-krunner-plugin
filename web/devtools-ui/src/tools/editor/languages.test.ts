import { describe, expect, it } from 'vitest';

import { FORMAT_DEFINITIONS } from '../converter/formats';
import { languageExtension, languageOfFormat } from './languages';
import type { EditorLanguageId } from './languages';

describe('languageOfFormat', () => {
  it('把结构化格式映射到对应语言', () => {
    expect(languageOfFormat('json')).toBe('json');
    expect(languageOfFormat('json-deep')).toBe('json');
    expect(languageOfFormat('json-min')).toBe('json');
    expect(languageOfFormat('js-object')).toBe('javascript');
    expect(languageOfFormat('yaml')).toBe('yaml');
    expect(languageOfFormat('xml')).toBe('xml');
    expect(languageOfFormat('toml')).toBe('toml');
  });

  it('把键值行格式映射到 properties 流模式', () => {
    expect(languageOfFormat('ini')).toBe('properties');
    expect(languageOfFormat('cookie')).toBe('properties');
    expect(languageOfFormat('postman-bulk')).toBe('properties');
  });

  it('把 PHP 输出格式映射到 php 语言', () => {
    expect(languageOfFormat('php-array')).toBe('php');
    expect(languageOfFormat('php-var-export')).toBe('php');
  });

  it('无法高亮的格式回落为纯文本', () => {
    expect(languageOfFormat('base64')).toBe('plain');
    expect(languageOfFormat('jwt')).toBe('plain');
    expect(languageOfFormat('csv')).toBe('plain');
    expect(languageOfFormat('uri')).toBe('plain');
    expect(languageOfFormat('plain')).toBe('plain');
  });

  it('每一种转换格式都能解析出可用语言', () => {
    const supported: readonly EditorLanguageId[] = [
      'javascript',
      'json',
      'php',
      'plain',
      'properties',
      'toml',
      'xml',
      'yaml',
    ];
    for (const definition of FORMAT_DEFINITIONS) {
      expect(supported).toContain(languageOfFormat(definition.id));
    }
  });
});

describe('languageExtension', () => {
  it('plain 返回 null，其余语言返回扩展实例', () => {
    expect(languageExtension('plain')).toBeNull();
    for (const language of [
      'javascript',
      'json',
      'php',
      'properties',
      'toml',
      'xml',
      'yaml',
    ] as const) {
      expect(languageExtension(language)).not.toBeNull();
    }
  });

  it('同一语言重复获取返回同一实例', () => {
    expect(languageExtension('json')).toBe(languageExtension('json'));
  });
});
