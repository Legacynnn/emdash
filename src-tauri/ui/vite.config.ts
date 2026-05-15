import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';
import { defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

// Roots for cross-package imports — the existing Electron renderer is
// the source of truth for product UI. Tauri's UI shell only adds a
// thin polyfill that exposes `window.electronAPI` over Tauri invoke.
const repoRoot = resolve(__dirname, '../..');
const srcRoot = resolve(repoRoot, 'src');

// Tauri-recommended Vite config: fixed port (mirrored in tauri.conf.json
// `devUrl`), no clearScreen so Cargo errors stay visible.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: {
      '@': srcRoot,
      '@renderer': resolve(srcRoot, 'renderer'),
      '@shared': resolve(srcRoot, 'shared'),
      '@main': resolve(srcRoot, 'main'),
      '@root': repoRoot,
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    fs: {
      // Vite locks file serving to its `root` by default. The renderer
      // and shared sources live above this package, so allow the repo
      // root explicitly.
      allow: [repoRoot],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
