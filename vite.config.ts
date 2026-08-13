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
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json-summary'],
      exclude: ['src/shared/api/bindings.ts'],
      thresholds: {
        statements: 70,
        branches: 70,
        functions: 65,
        lines: 70,
      },
    },
  },
  build: {
    target: 'es2022',
    sourcemap: true,
    chunkSizeWarningLimit: 350,
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [{
            name: 'framework',
            test: /node_modules[\\/](?:@lucide[\\/]vue|@tauri-apps|@vue|vue|vue-router)[\\/]/,
          }],
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
})
