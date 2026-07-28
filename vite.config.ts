import { fileURLToPath, URL } from 'node:url'
import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
    css: true,
    exclude: ['.worktrees/**', 'node_modules/**', 'dist/**', 'src-tauri/**'],
  },
  build: {
    target: 'es2022',
    sourcemap: true,
    chunkSizeWarningLimit: 350,
  },
  server: {
    port: 1420,
    strictPort: true,
  },
})
