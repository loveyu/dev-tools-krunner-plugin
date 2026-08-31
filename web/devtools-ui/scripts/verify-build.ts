import { readdir, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const distributionDirectory = resolve(import.meta.dirname, '../dist');
const files = await readdir(distributionDirectory);

if (files.length !== 1 || files[0] !== 'index.html') {
  throw new Error(`前端产物必须是单个 index.html，实际为：${files.join(', ')}`);
}

const html = await readFile(resolve(distributionDirectory, 'index.html'), 'utf8');
const forbiddenExternalAsset = /<(?:script|link)\b[^>]+(?:src|href)=["'][^"']+["']/iu;

if (forbiddenExternalAsset.test(html)) {
  throw new Error('前端产物仍引用外部脚本或样式，无法安全嵌入 Worker');
}

if (!html.includes('id="app"') || html.length < 10_000) {
  throw new Error('前端产物缺少应用挂载点或内容异常');
}

console.log(`已验证单文件 WebView 产物：${html.length.toLocaleString('zh-CN')} 字节`);
