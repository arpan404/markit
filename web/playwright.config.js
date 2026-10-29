import { defineConfig } from '@playwright/test';
// CHROMIUM_PATH points at a preinstalled browser when the bundled revision is absent.
const launchOptions = process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {};
export default defineConfig({ testDir: './tests', testIgnore: ['gpu.spec.js', 'node/**'], globalSetup: './tests/react/build.mjs', use: { baseURL: 'http://127.0.0.1:4173', headless: true, launchOptions }, webServer: { command: 'python3 -m http.server 4173 --bind 127.0.0.1', port: 4173, reuseExistingServer: true }, workers: 1 });
