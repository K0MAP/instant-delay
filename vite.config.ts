import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ['react', 'react-dom', 'zustand'],
          tauri: ['@tauri-apps/api', '@tauri-apps/plugin-store'],
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Tauri watches Rust sources itself; build artifacts can be locked on Windows.
      ignored: ['**/src-tauri/**'],
    },
    hmr: {
      port: 1421,
    },
  },
});