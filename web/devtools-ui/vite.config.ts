import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vitest/config';
import { viteSingleFile } from 'vite-plugin-singlefile';

export default defineConfig({
  base: './',
  plugins: [vue(), viteSingleFile()],
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
