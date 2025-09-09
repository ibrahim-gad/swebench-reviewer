import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginSass } from '@rsbuild/plugin-sass';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [pluginReact(), pluginSass()],
  
  // HTML template configuration
  html: {
    template: './index.html',
  },
  
  // Entry point configuration
  source: {
    entry: {
      index: './src/main.tsx',
    },
  },
  
  // Server configuration for Tauri development
  server: {
    port: 1420,
    strictPort: true,
    host: host || 'localhost',
  },
  
  // Development configuration
  dev: {
    hmr: true,
  },
  
  // Build target configuration for Tauri
  output: {
    distPath: {
      root: 'dist',
    },
  },
  
  // Performance and compatibility settings
  performance: {
    // Don't clear screen to see Rust errors
    printFileSize: true,
  },
}); 