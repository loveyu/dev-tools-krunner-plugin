import { describe, expect, it, vi } from 'vitest';

import { WEB_CODECS } from './codecs';
import { convertText } from './core';
import { detectFormat } from './detect';
import { FORMAT_DEFINITIONS } from './formats';
import type { FormatId, NativeExecutor, WebCodec } from './types';

const unexpectedNativeCall: NativeExecutor = (): Promise<string> =>
  Promise.reject(new Error('不应调用 native 转换器'));

describe('convertText', () => {
  it('在纯 Web 转换器之间保留结构和值类型', async () => {
    const output = await convertText(
      '{"zero":0,"disabled":false,"nested":"{\\"ok\\":true}"}',
      'json-deep',
      'yaml',
      unexpectedNativeCall,
    );

    expect(output).toContain('zero: 0');
    expect(output).toContain('disabled: false');
    expect(output).toContain('ok: true');
  });

  it('通过异步 native 边界解析和输出 PHP 格式', async () => {
    const native = vi.fn<NativeExecutor>((request) => {
      if (request.direction === 'parse') return Promise.resolve('{"name":"devtools"}');
      return Promise.resolve(`serialized:${request.payload}`);
    });

    const output = await convertText('a:1:{...}', 'php-serialize', 'php-array', native);

    expect(output).toBe('serialized:{"name":"devtools"}');
    expect(native).toHaveBeenNthCalledWith(1, {
      direction: 'parse',
      format: 'php-serialize',
      payload: 'a:1:{...}',
    });
    expect(native).toHaveBeenNthCalledWith(2, {
      direction: 'stringify',
      format: 'php-array',
      payload: '{"name":"devtools"}',
    });
  });

  it('拒绝把仅输出和仅输入格式放错方向', async () => {
    await expect(convertText('{}', 'json-min', 'json', unexpectedNativeCall)).rejects.toThrow(
      'convert.errors.sourceFormatNotParsable',
    );
    await expect(convertText('{}', 'json', 'toml', unexpectedNativeCall)).rejects.toThrow(
      'convert.errors.targetFormatNotStringifiable',
    );
  });
});

describe('Web codecs', () => {
  it('支持 JSON5、XML、CSV、Query 与 Cookie 的主链路', () => {
    expect(parse('js-object', "{name: 'devtools', enabled: true}")).toEqual({
      name: 'devtools',
      enabled: true,
    });
    expect(parse('xml', '<root enabled="true"><name>devtools</name></root>')).toEqual({
      root: { '@_enabled': true, name: 'devtools' },
    });
    expect(parse('csv', 'name,enabled\ndevtools,true')).toEqual([
      { name: 'devtools', enabled: 'true' },
    ]);
    expect(parse('query-rfc3986', 'name=devtools&tag%5B0%5D=kde')).toEqual({
      name: 'devtools',
      tag: ['kde'],
    });
    expect(parse('cookie', 'name=devtools; desktop=KDE')).toEqual({
      name: 'devtools',
      desktop: 'KDE',
    });
  });

  it('支持 Postman、行文本、URI 和 JWT', () => {
    expect(parse('postman-bulk', 'name:devtools\nnote:hello↵world')).toEqual({
      name: 'devtools',
      note: 'hello\nworld',
    });
    expect(parse('line', 'one\r\ntwo')).toEqual(['one', 'two']);
    expect(parse('uri', 'https://u:p@example.com:8443/a?x=1#part')).toMatchObject({
      scheme: 'https',
      host: 'example.com',
      port: 8443,
      user: 'u',
      pass: 'p',
      path: '/a',
      fragment: 'part',
    });
    expect(parse('jwt', 'eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0.signature')).toMatchObject({
      headers: { alg: 'none' },
      claims: { sub: '1' },
      signature: 'signature',
    });
  });

  it('支持 UTF-8 Base64、Gzip 和 URL Encode 往返', () => {
    for (const format of ['base64', 'base64-gzip', 'url-encode'] as const) {
      const encoded = stringify(format, '你好 DevTools');
      expect(parse(format, encoded)).toBe('你好 DevTools');
    }
  });

  it('拒绝 XML DOCTYPE 和原型污染键', () => {
    expect(() => parse('xml', '<!DOCTYPE root><root/>')).toThrow(
      'convert.errors.xmlDoctypeForbidden',
    );
    expect(() => parse('json', '{"__proto__":{"polluted":true}}')).toThrow(
      'convert.errors.forbiddenObjectKey',
    );
  });
});

describe('detectFormat', () => {
  const available = new Set(
    FORMAT_DEFINITIONS.filter((definition) => definition.canParse).map(
      (definition) => definition.id,
    ),
  );

  it.each<[string, FormatId]>([
    ['{"name":"devtools"}', 'json-deep'],
    ["{name: 'devtools'}", 'js-object'],
    ['<root><name>devtools</name></root>', 'xml'],
    ['https://example.com/a?x=1', 'uri'],
    ['a:1:{s:1:"a";i:1;}', 'php-serialize'],
    ['name=devtools&desktop=kde', 'query-rfc3986'],
    ['name,desktop\ndevtools,kde', 'csv'],
    ['name\tdesktop\ndevtools\tkde', 'tsv'],
    ['name:devtools\ndesktop:kde', 'postman-bulk'],
    ['[owner]\nname = "devtools"', 'toml'],
    ['[owner]\nname=devtools', 'ini'],
    ['name: devtools', 'yaml'],
    ['SGVsbG8=', 'base64'],
    ['hello%20world', 'url-encode'],
    ['ordinary text', 'plain'],
  ])('把 %s 探测为 %s', (text, expected) => {
    expect(detectFormat(text, available)).toBe(expected);
  });

  it('不会返回当前运行环境不可用的 PHP 来源', () => {
    const webOnly = new Set([...available].filter((format) => format !== 'php-serialize'));
    expect(detectFormat('a:1:{s:1:"a";i:1;}', webOnly)).toBe('plain');
  });
});

function codec(format: FormatId): WebCodec {
  const value = WEB_CODECS[format];
  if (value === undefined) throw new Error(`测试格式没有 codec：${format}`);
  return value;
}

function parse(format: FormatId, text: string): unknown {
  const parser = codec(format).parse;
  if (parser === undefined) throw new Error(`测试格式不支持解析：${format}`);
  return parser(text);
}

function stringify(format: FormatId, value: string): string {
  const generator = codec(format).stringify;
  if (generator === undefined) throw new Error(`测试格式不支持输出：${format}`);
  return generator(value);
}
