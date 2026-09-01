import vue from '@vitejs/plugin-vue';
import type { Plugin } from 'vite';
import { defineConfig } from 'vitest/config';
import { viteSingleFile } from 'vite-plugin-singlefile';

// index.html 的 CSP 面向嵌入 Worker 的单文件产物（禁止一切外部资源），
// 但它会阻断 Vite 开发服务器的模块加载与 HMR WebSocket，因此 dev 模式下移除。
// 另外 fast-xml-validator 的传递依赖 @nodable/flexible-xml-parser 在模块顶层
// 调用 Node 的 Buffer.from 生成 BOM 字节：生产构建会摇除该路径，而 dev 的
// 依赖预打包原样执行，需要注入仅够字节场景使用的最小 Buffer 实现。
const BUFFER_SHIM = `<script>
  if (typeof globalThis.Buffer === 'undefined') {
    globalThis.Buffer = {
      from(value) {
        if (Array.isArray(value)) return Uint8Array.from(value);
        return new TextEncoder().encode(String(value));
      },
    };
  }
</script>`;

function devRuntimeShims(): Plugin {
  return {
    name: 'devtools-dev-runtime-shims',
    apply: 'serve',
    transformIndexHtml(html: string): string {
      return html
        .replace(/\s*<meta\s+http-equiv="Content-Security-Policy"[^>]*>/u, '')
        .replace('</head>', `  ${BUFFER_SHIM}\n</head>`);
    },
  };
}

export default defineConfig({
  base: './',
  plugins: [vue(), viteSingleFile(), devRuntimeShims()],
  build: {
    cssMinify: 'lightningcss',
    target: 'es2022',
  },
  test: {
    environment: 'node',
    coverage: {
      provider: 'v8',
      include: [
        'src/i18n/core.ts',
        'src/tools/json/model.ts',
        'src/tools/launcher/model.ts',
        'src/tools/image-compression/model.ts',
        'src/tools/image-editor/config.ts',
        'src/tools/image-editor/export.ts',
        'src/tools/media/barcode-generator.ts',
        'src/tools/media/image.ts',
        'src/tools/media/result.ts',
        'src/tools/watermark/model.ts',
        'src/tools/color/model.ts',
        'src/tools/crypto/model.ts',
        'src/tools/editor/languages.ts',
        'src/tools/editor/phrases.ts',
      ],
      thresholds: {
        branches: 100,
        functions: 100,
        lines: 100,
        statements: 100,
      },
    },
  },
});
