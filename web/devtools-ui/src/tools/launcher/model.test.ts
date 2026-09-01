import { describe, expect, it } from 'vitest';

import { matchLauncherQuery } from './model';

describe('matchLauncherQuery', () => {
  it('把直接输入的 JSON 对象识别为 JSON 工作台请求', () => {
    const matches = matchLauncherQuery('  {"ok":true}  ');

    expect(matches).toHaveLength(1);
    expect(matches[0]?.action).toEqual({
      type: 'open-tool',
      tool: 'json',
      payload: '{"ok":true}',
    });
  });

  it('把直接输入的 JSON 数组识别为 JSON 工作台请求', () => {
    expect(matchLauncherQuery('[1,2]')[0]?.id).toBe('json');
  });

  it('不会把无效或标量 JSON 当作直接 JSON', () => {
    expect(matchLauncherQuery('{broken')).toEqual([]);
    expect(matchLauncherQuery('true')).toEqual([]);
  });

  it('支持命令别名和其后的文本载荷', () => {
    expect(matchLauncherQuery('cv a=1 b=2')[0]?.action).toEqual({
      type: 'open-tool',
      tool: 'convert',
      payload: 'a=1 b=2',
    });
    expect(matchLauncherQuery('qr')[0]?.id).toBe('barcode');
  });

  it('精确匹配设置动作', () => {
    expect(matchLauncherQuery('config')[0]?.action).toEqual({ type: 'open-settings' });
  });

  it('空输入列出所有入口并支持前缀检索', () => {
    expect(matchLauncherQuery('')).toHaveLength(11);
    expect(matchLauncherQuery('squ')[0]?.id).toBe('image-compress');
    expect(matchLauncherQuery('图片')[0]?.id).toBe('image-compress');
    expect(matchLauncherQuery('watermarker')[0]?.id).toBe('watermark');
    expect(matchLauncherQuery('encrypt')[0]?.id).toBe('crypto');
    expect(matchLauncherQuery('exiftool')[0]?.id).toBe('metadata');
    expect(matchLauncherQuery('eyedropper')[0]?.id).toBe('color');
  });
});
