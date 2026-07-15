import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 120000, // 2 minutes per test to accommodate slow compilation on WSL
  expect: {
    timeout: 15000,
  },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:5173',
    trace: 'on-first-retry',
    actionTimeout: 30000,
    navigationTimeout: 60000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: [
    {
      command: '../target/release/kendra run ui',
      url: 'http://127.0.0.1:8080/api/health',
      reuseExistingServer: !process.env.CI,
      timeout: 120000,
    },
    {
      command: 'node node_modules/vite/bin/vite.js --port 5173 --host 127.0.0.1',
      url: 'http://127.0.0.1:5173',
      reuseExistingServer: !process.env.CI,
      timeout: 120000,
    }
  ],
});
