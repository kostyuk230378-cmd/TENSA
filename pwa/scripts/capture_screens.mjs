/**
 * Съёмка скриншотов PWA Tensa через headless Edge и Chrome DevTools Protocol.
 *
 * Зачем: PWA раздаётся сервером на localhost, поэтому снимать её надо
 * системным браузером; скрипт заодно прогоняет живой сценарий (ввод токена,
 * клики по навигации, создание заметки через настоящую форму) — это
 * визуальная проверка интерфейса и заготовка материалов для демо.
 *
 * Запуск:
 *   node pwa/scripts/capture_screens.mjs <edge.exe> <baseUrl> <token> <outDir>
 *
 * Пример:
 *   node pwa/scripts/capture_screens.mjs \
 *     "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" \
 *     http://127.0.0.1:8765/app demo-token pwa/screenshots
 *
 * Требуется запущенный tensa-server (см. crates/tensa-server/README.md).
 */

import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [edgePath, baseUrl, token, outDir] = process.argv.slice(2);
if (!edgePath || !baseUrl || !token || !outDir) {
  console.error('usage: node capture_screens.mjs <edge.exe> <baseUrl> <token> <outDir>');
  process.exit(2);
}
mkdirSync(outDir, { recursive: true });

const port = 9223;
const profile = join(tmpdir(), `tensa-capture-${Date.now()}`);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const edge = spawn(
  edgePath,
  [
    '--headless=new',
    '--disable-gpu',
    '--no-first-run',
    '--hide-scrollbars',
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`,
    '--window-size=430,932',
    baseUrl,
  ],
  { stdio: 'ignore' },
);

/** Дождаться страничной цели CDP. */
async function findTarget() {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      const targets = await response.json();
      const page = targets.find((item) => item.type === 'page' && item.url.startsWith('http'));
      if (page?.webSocketDebuggerUrl) {
        return page;
      }
    } catch {
      // CDP ещё не поднялся — повторим.
    }
    await sleep(250);
  }
  throw new Error('CDP не поднялся');
}

/** Мини-клиент CDP поверх WebSocket (Node 24: глобальный WebSocket). */
function connect(url) {
  const socket = new WebSocket(url);
  let nextId = 0;
  const pending = new Map();
  const ready = new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = () => reject(new Error('не удалось подключиться к CDP'));
  });
  socket.onmessage = (event) => {
    const message = JSON.parse(String(event.data));
    const resolve = pending.get(message.id);
    if (resolve) {
      pending.delete(message.id);
      resolve(message);
    }
  };
  async function send(method, params = {}) {
    await ready;
    const id = (nextId += 1);
    return new Promise((resolve, reject) => {
      pending.set(id, (message) =>
        message.error ? reject(new Error(`${method}: ${message.error.message}`)) : resolve(message.result),
      );
      socket.send(JSON.stringify({ id, method, params }));
    });
  }
  return { send, close: () => socket.close() };
}

try {
  const target = await findTarget();
  const cdp = connect(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');

  /** Выполнить выражение в странице и вернуть значение. */
  async function evaluate(expression) {
    const result = await cdp.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (result.exceptionDetails) {
      throw new Error(`eval: ${result.exceptionDetails.text}`);
    }
    return result.result?.value;
  }

  /** Сохранить скриншот страницы. */
  async function shot(name) {
    const { data } = await cdp.send('Page.captureScreenshot', { format: 'png' });
    const path = join(outDir, name);
    writeFileSync(path, Buffer.from(data, 'base64'));
    console.log(`снимок: ${path}`);
  }

  // 1. Токен в localStorage + перезагрузка: приложение выходит на рабочие экраны.
  await evaluate(`localStorage.setItem('tensa.pwa.access', ${JSON.stringify(token)}); true`);
  try {
    await evaluate('location.reload(); true');
  } catch {
    // Перезагрузка рвёт текущий execution context — это ожидаемо.
  }
  await sleep(3000);

  const status = await evaluate(`({
    nav: document.querySelectorAll('nav button').length,
    heading: document.querySelector('main h2')?.textContent?.trim() ?? '',
    text: (document.querySelector('main')?.innerText ?? '').split('\\n').slice(0, 6).join(' | '),
    banner: document.querySelector('.banner')?.innerText ?? '',
  })`);
  console.log('статус:', JSON.stringify(status, null, 2));
  await shot('02-status.png');

  // 2. Одобрения (очередь пуста — честный текст).
  await evaluate(`document.querySelectorAll('nav button')[1]?.click(); true`);
  await sleep(700);
  console.log('одобрения:', await evaluate(`document.querySelector('main')?.innerText?.slice(0, 160) ?? ''`));
  await shot('03-approvals.png');

  // 3. Заметки: создание через настоящую форму (Svelte bind:value — событие input).
  await evaluate(`document.querySelectorAll('nav button')[2]?.click(); true`);
  await sleep(700);
  await evaluate(`(() => {
    const title = document.getElementById('note-title');
    const body = document.getElementById('note-body');
    if (title) { title.value = 'Заметка из браузера'; title.dispatchEvent(new Event('input', { bubbles: true })); }
    if (body) { body.value = 'Создана headless-браузером через настоящую форму.'; body.dispatchEvent(new Event('input', { bubbles: true })); }
    return true;
  })()`);
  await sleep(300);
  await evaluate(`[...document.querySelectorAll('button')].find((b) => b.textContent.includes('Сохранить'))?.click(); true`);
  await sleep(900);
  console.log('заметки:', await evaluate(`document.querySelector('main')?.innerText?.slice(0, 220) ?? ''`));
  await shot('04-notes.png');

  // 4. Поиск по созданной заметке.
  await evaluate(`document.querySelectorAll('nav button')[3]?.click(); true`);
  await sleep(600);
  await evaluate(`(() => {
    const input = document.querySelector('.searchbar input');
    if (input) { input.value = 'headless'; input.dispatchEvent(new Event('input', { bubbles: true })); }
    const form = document.querySelector('.searchbar');
    if (form) { form.requestSubmit ? form.requestSubmit() : form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); }
    return true;
  })()`);
  await sleep(900);
  console.log('поиск:', await evaluate(`document.querySelector('main')?.innerText?.slice(0, 220) ?? ''`));
  await shot('05-search.png');

  // 5. Настройки: перед кадром очищаем поле токена в форме (токен не должен
  // попадать в демо-скриншоты; в localStorage значение не затрагивается).
  await evaluate(`document.querySelectorAll('nav button')[5]?.click(); true`);
  await sleep(600);
  await evaluate(`(() => {
    const input = document.getElementById('settings-token');
    if (input) { input.value = ''; input.dispatchEvent(new Event('input', { bubbles: true })); }
    return true;
  })()`);
  await sleep(200);
  await shot('06-settings.png');

  cdp.close();
  console.log('готово');
} finally {
  edge.kill();
}
