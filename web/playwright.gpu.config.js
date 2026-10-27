import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir:'./tests',testMatch:'gpu.spec.js',workers:1,
  use:{baseURL:'http://127.0.0.1:4173',headless:process.env.MARKIT_GPU_HEADLESS === '1'},
  webServer:{command:'python3 -m http.server 4173 --bind 127.0.0.1',port:4173,reuseExistingServer:true}
});
