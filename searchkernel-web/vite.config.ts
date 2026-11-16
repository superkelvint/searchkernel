import { defineConfig, searchForWorkspaceRoot } from 'vite'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  // W6 consumes the canonical InstantSearch adapter source/declaration directly.
  // Keep dev-server filesystem access scoped to that package rather than the repo.
  server: { fs: { allow: [searchForWorkspaceRoot(fileURLToPath(new URL('.', import.meta.url))), fileURLToPath(new URL('../integrations/instantsearch', import.meta.url))] } },
  // The public SDK is a linked CommonJS package outside node_modules.
  optimizeDeps: { include: ['@searchkernel/client'] },
  build: { commonjsOptions: { include: [/node_modules/, /clients\/typescript\/dist/] } },
})
