import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 2703,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'oxc' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      // Vite treats any chunk named "-legacy" as a legacy bundle and injects
      // its CSS inline, which the packaged Linux webview's CSP rejects.
      ...(process.platform === 'linux' ? {
        output: {
          chunkFileNames: (chunk: { name: string }) =>
            `assets/${chunk.name.replace(/-legacy/g, '-styles')}-[hash].js`,
        },
      } : {}),
      input: {
        main: resolve(__dirname, 'index.html'),
        editor: resolve(__dirname, 'editor.html'),
        preview: resolve(__dirname, 'preview.html'),
        'recording-hud': resolve(__dirname, 'recording-hud.html'),
        webcam: resolve(__dirname, 'webcam.html'),
      },
    },
  },
});
