/**
 * Диагностика WebView2 по CDP: исключения JS и состояние страницы.
 * Запуск: node webview_debug.mjs <port>
 */
const port = process.argv[2] ?? '9222';

async function targets() {
  const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  return list.filter((item) => item.type === 'page');
}

function connect(url) {
  const socket = new WebSocket(url);
  let nextId = 0;
  const pending = new Map();
  const events = [];
  socket.onmessage = (event) => {
    const message = JSON.parse(String(event.data));
    if (message.method) events.push(message);
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    if (message.error) entry.reject(new Error(message.error.message));
    else entry.resolve(message.result);
  };
  const ready = new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = () => reject(new Error('CDP connect'));
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = (nextId += 1);
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
  return { ready, send, events, close: () => socket.close() };
}

const list = await targets();
console.log('targets:', list.map((t) => t.url));
if (!list.length) process.exit(1);
const cdp = connect(list[0].webSocketDebuggerUrl);
await cdp.ready;
await cdp.send('Runtime.enable');
await cdp.send('Page.enable');
await cdp.send('Page.reload', { ignoreCache: true });
await new Promise((r) => setTimeout(r, 2500));

const state = await cdp.send('Runtime.evaluate', {
  expression: `(() => ({
    href: location.href,
    letter: document.querySelector('.letter')?.textContent ?? null,
    scripts: document.scripts.length,
    marker: document.querySelector('.letter')?.title ?? '',
    ready: document.readyState,
  }))()`,
  returnByValue: true,
});
console.log('state:', JSON.stringify(state.result?.value));

const exceptions = cdp.events
  .filter((event) => event.method === 'Runtime.exceptionThrown')
  .map((event) => event.params.exceptionDetails?.exception?.description ?? event.params.exceptionDetails?.text);
console.log('exceptions:', exceptions);

const test = await cdp.send('Runtime.evaluate', {
  expression: `(() => { document.querySelector('.letter').textContent = 'X'; return 'ok'; })()`,
  returnByValue: true,
});
console.log('js works:', test.result?.value);
cdp.close();
