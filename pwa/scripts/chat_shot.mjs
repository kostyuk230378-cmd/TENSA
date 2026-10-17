/**
 * Точечный скриншот экрана «Чат»: открывает PWA, вводит токен, шлёт сообщение
 * в ядро и ждёт ответа (локальная GGUF может думать долго).
 *
 * Запуск: node pwa/scripts/chat_shot.mjs <edge.exe> <baseUrl> <token> <outFile>
 */
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [edgePath, baseUrl, token, outFile] = process.argv.slice(2);
if (!edgePath || !baseUrl || !token || !outFile) {
  console.error('usage: node chat_shot.mjs <edge.exe> <baseUrl> <token> <outFile>');
  process.exit(2);
}

const port = 9224;
const profile = join(tmpdir(), `tensa-chat-${Date.now()}`);
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
      // CDP ещё не поднялся
    }
    await sleep(250);
  }
  throw new Error('CDP не поднялся');
}

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
    const entry = pending.get(message.id);
    if (!entry) {
      return;
    }
    pending.delete(message.id);
    if (message.error) {
      entry.reject(new Error(message.error.message));
    } else {
      entry.resolve(message.result);
    }
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = (nextId += 1);
      pending.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params }));
    });
  return { ready, send, close: () => socket.close() };
}

try {
  const target = await findTarget();
  const cdp = connect(target.webSocketDebuggerUrl);
  await cdp.ready;
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');

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

  await evaluate(`localStorage.setItem('tensa.pwa.access', ${JSON.stringify(token)}); true`);
  try {
    await evaluate('location.reload(); true');
  } catch {
    // перезагрузка рвёт execution context — ожидаемо
  }
  await sleep(3000);
  await evaluate(`document.querySelectorAll('nav button')[0]?.click(); true`);
  await sleep(800);
  console.log('экран:', await evaluate(`document.querySelector('main h2')?.textContent ?? ''`));

  await evaluate(`(() => {
    const input = document.querySelector('main textarea');
    if (!input) { return false; }
    input.value = 'Скажи одним предложением, кто ты.';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    return true;
  })()`);
  await sleep(300);
  await evaluate(`[...document.querySelectorAll('main button')].find((b) => b.textContent.includes('Отправить'))?.click(); true`);
  console.log('сообщение отправлено, жду ядро…');

  let text = '';
  for (let attempt = 0; attempt < 120; attempt += 1) {
    await sleep(2000);
    text = await evaluate(`document.querySelector('main')?.innerText ?? ''`);
    if (attempt % 5 === 0) {
      console.log(`[${attempt * 2}s]`, text.replace(/\\s+/g, ' ').slice(0, 140));
    }
    const bubbles = await evaluate(`document.querySelectorAll('main .bubble').length`);
    if (bubbles >= 2 && !text.includes('печатает')) {
      break;
    }
  }
  const { data } = await cdp.send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(outFile, Buffer.from(data, 'base64'));
  console.log('снимок:', outFile);
  console.log('итог:', text.replace(/\\s+/g, ' ').slice(0, 260));
  cdp.close();
} finally {
  edge.kill();
}
