import { defineConfig } from '@playwright/test';

/// Playwright-конфиг E2E-тестов PWA (02-testing.md, Фаза 5).
/// Предусловия: сервер ядра на http://127.0.0.1:8765, браузер Chromium
/// установлен (`npx playwright install chromium`), токен в TENSA_E2E_TOKEN.
export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  use: {
    browserName: 'chromium',
    headless: true,
  },
  // Сервер ядра и браузер запускаются вручную (см. e2e/approve.spec.ts).
});
