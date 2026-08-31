import { describe, expect, it } from 'vitest';

import {
  buildJsonTree,
  countMatches,
  filterJsonTree,
  formatJson,
  minifyJson,
  parseJson,
} from './model';

describe('JSON 模型', () => {
  it('解析、格式化并压缩 JSON', () => {
    const value = parseJson('{"name":"loveyu","items":[1,2]}');

    expect(formatJson(value)).toBe('{\n  "name": "loveyu",\n  "items": [\n    1,\n    2\n  ]\n}');
    expect(minifyJson(value)).toBe('{"name":"loveyu","items":[1,2]}');
  });

  it('拒绝语法无效的 JSON', () => {
    expect(() => parseJson('{invalid}')).toThrow(SyntaxError);
  });

  it('拒绝 JSON 以外的非有限数值', () => {
    const originalParse = JSON.parse;
    JSON.parse = (): number => Number.POSITIVE_INFINITY;
    try {
      expect(() => parseJson('0')).toThrow(TypeError);
    } finally {
      JSON.parse = originalParse;
    }
  });

  it('构造包含对象、数组和安全路径的树', () => {
    const tree = buildJsonTree(parseJson('{"normal":true,"a-b":[null]}'));

    expect(tree.preview).toBe('{2}');
    expect(tree.children[0]?.path).toBe('$.normal');
    expect(tree.children[1]?.path).toBe('$["a-b"]');
    expect(tree.children[1]?.preview).toBe('[1]');
  });

  it('按键、路径或值筛选，并保留命中节点的祖先', () => {
    const tree = buildJsonTree(parseJson('{"user":{"name":"loveyu"},"active":true}'));
    const filtered = filterJsonTree(tree, 'LOVEYU');

    expect(filtered?.children).toHaveLength(1);
    expect(filtered?.children[0]?.children[0]?.key).toBe('name');
    expect(countMatches(tree, 'user')).toBe(2);
  });

  it('空查询返回完整树且不计命中，未命中返回 null', () => {
    const tree = buildJsonTree(parseJson('[1]'));

    expect(filterJsonTree(tree, ' ')).toBe(tree);
    expect(countMatches(tree, '')).toBe(0);
    expect(filterJsonTree(tree, 'missing')).toBeNull();
  });
});
