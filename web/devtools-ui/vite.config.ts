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
        'src/tools/json/model.ts',
        'src/tools/media/barcode-generator.ts',
        'src/tools/media/image.ts',
        'src/tools/media/result.ts',
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
