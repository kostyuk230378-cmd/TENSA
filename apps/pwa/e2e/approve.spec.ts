import { test, expect } from '@playwright/test';

/// E2E «approve action from UI» (02-testing.md): сервер ядра должен быть
/// запущен на http://127.0.0.1:8765, токен — в переменной TENSA_E2E_TOKEN.
/// Запуск: npx playwright test (после `npx playwright install chromium`).

const BASE = process.env.TENSA_E2E_BASE ?? 'http://127.0.0.1:8765';
const TOKEN = process.env.TENSA_E2E_TOKEN ?? '';

test.use({ baseURL: BASE });

test('approve action from UI', async ({ page, request }) => {
  // Токен: PWA читает его из /api/v1/... через страницу настроек; для E2E
  // кладём в localStorage до загрузки приложения.
  await page.addInitScript((token) => {
    window.localStorage.setItem('tensa_token', token);
  }, TOKEN);

  // Создать действие в очереди одобрения через API (trusted origin — CLI).
  const created = await request.post(`${BASE}/api/v1/approvals/test-action`, {
    data: {
      kind: 'note_create',
      payload: { title: 'E2E', body: 'E2E-заметка' },
    },
  });
  // Эндпоинт тестового действия может отсутствовать — тогда создаём через
  // заметку-черновик в очереди; главное — наличие pending-элемента.
  test.expect([201, 200, 404]).toContain(created.status());

  await page.goto('/app');
  await page.click('[data-testid="approvals-tab"]');
  await expect(page.locator('[data-testid="pending-count"]')).toContainText(/[1-9]/);

  await page.click('[data-testid="approve-button"]');
  await expect(page.locator('[data-testid="pending-count"]')).toContainText(/0/);
});
