import { defineConfig, devices } from '@playwright/test';

const externalServer = process.env.DEK_PLAYWRIGHT_EXTERNAL_SERVER === '1';
const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? (
  externalServer ? 'http://127.0.0.1:3000' : 'http://127.0.0.1:5173'
);

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 1,
  workers: process.env.CI ? 1 : undefined,
  reporter: [['html'], ['junit', { outputFile: 'playwright-report/results.xml' }]],
  use: {
    baseURL,
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
