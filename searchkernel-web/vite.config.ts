import { defineConfig } from 'vite'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  // The public SDK is a linked CommonJS package outside node_modules.
  optimizeDeps: { include: ['@searchkernel/client'] },
  build: { commonjsOptions: { include: [/node_modules/, /clients\/typescript\/dist/] } },
})
