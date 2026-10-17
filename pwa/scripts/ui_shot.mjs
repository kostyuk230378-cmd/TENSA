/**
 * Скриншот главного экрана без токена (локальный вход) — проверка «чат-первого» UI.
 * Запуск: node pwa/scripts/ui_shot.mjs <edge.exe> <baseUrl> <outFile>
 */
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [edgePath, baseUrl, outFile] = process.argv.slice(2);
const port = 9226;
const profile = join(tmpdir(), `tensa-ui-${Date.now()}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const edge = spawn(edgePath, [
  '--headless=new', '--disable-gpu', '--no-first-run', '--hide-scrollbars',
  `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
  '--window-size=460,860', baseUrl,
], { stdio: 'ignore' });

async function findTarget() {
  for (let i = 0; i < 80; i += 1) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      const page = list.find((t) => t.type === 'page' && t.url.startsWith('http'));
      if (page?.webSocketDebuggerUrl) return page;
    } catch { /* ещё не поднялся */ }
    await sleep(250);
  }
  throw new Error('CDP не поднялся');
}

function connect(url) {
  const socket = new WebSocket(url);
  let nextId = 0;
  const pending = new Map();
  socket.onmessage = (event) => {
    const message = JSON.parse(String(event.data));
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    if (message.error) entry.reject(new Error(message.error.message));
    else entry.resolve(message.result);
  };
  const ready = new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = () => reject(new Error('CDP'));
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
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
  await sleep(3500);
  const state = await cdp.send('Runtime.evaluate', {
    expression: `({ title: document.querySelector('main h2')?.textContent ?? '', nav: [...document.querySelectorAll('nav.bottom button')].map((b) => b.textContent.trim()), firstrun: !!document.querySelector('input[type=password]') })`,
    returnByValue: true,
  });
  console.log('состояние:', JSON.stringify(state.result?.value));
  const { data } = await cdp.send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(outFile, Buffer.from(data, 'base64'));
  console.log('снимок:', outFile);
  cdp.close();
} finally {
  edge.kill();
}
