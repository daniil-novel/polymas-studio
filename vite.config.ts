import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  root: 'apps/desktop',
  build: { outDir: '../../dist', emptyOutDir: true, target: 'es2022' },
  server: { port: 1420, strictPort: true, proxy: { '/api': 'http://127.0.0.1:8787' } },
  test: { root: '.', include: ['tests/frontend/**/*.test.{ts,tsx}'], environment: 'jsdom' },
});
