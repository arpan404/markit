import { defineConfig } from '@playwright/test';
export default defineConfig({ testDir: './tests', testIgnore: 'gpu.spec.js', globalSetup: './tests/react/build.mjs', use: { baseURL: 'http://127.0.0.1:4173', headless: true }, webServer: { command: 'python3 -m http.server 4173 --bind 127.0.0.1', port: 4173, reuseExistingServer: true }, workers: 1 });
