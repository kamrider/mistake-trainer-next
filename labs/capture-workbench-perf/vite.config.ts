import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [vue()],
  build: {
    emptyOutDir: true,
    outDir: '../../.test-tmp/capture-workbench-perf-dist',
  },
  server: {
    fs: {
      allow: ['../..'],
    },
  },
})
