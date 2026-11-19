import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests/scenarios',
  testMatch: '**/*.spec.mjs',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: process.env.CI ? 2 : 1,
  timeout: 90_000,
  expect: { timeout: 15_000 },
  outputDir: 'test-results/browser',
  reporter: [['list'], ['html', { outputFolder: 'playwright-report', open: 'never' }], ['junit', { outputFile: 'test-results/browser.xml' }]],
  use: {
    baseURL: 'http://127.0.0.1:15187',
    viewport: { width: 1440, height: 900 },
    locale: 'en-US',
    deviceScaleFactor: 1,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium' } },
    // WebKit on the Linux runners occasionally loses its web content process while the dev host
    // loads the engine and opens the sample (the page stops responding, then "Target crashed"),
    // before any scenario step runs. CI retries WebKit so such a run is reported as flaky; a
    // scenario that fails every attempt still fails the run. Local runs never retry.
    { name: 'webkit', retries: process.env.CI ? 2 : 0, use: { browserName: 'webkit' } },
  ],
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 15187 --strictPort',
    url: 'http://127.0.0.1:15187/dev.html',
    reuseExistingServer: false,
    timeout: 60_000,
  },
})
