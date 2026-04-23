import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';
import { mkdirSync } from 'node:fs';

const outRoot = resolve(__dirname, '../system-apps-dist');

mkdirSync(outRoot, { recursive: true });

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: outRoot,
    emptyOutDir: false,
    rollupOptions: {
      input: {
        'task-manager': resolve(__dirname, 'src/task-manager/index.html'),
      },
    },
  },
});
