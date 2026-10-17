/** Диагностика IPC виджета: setExpanded и window.ipc. */
const port = process.argv[2] ?? '9222';
const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const ws = new WebSocket(list[0].webSocketDebuggerUrl);
let id = 0;
const pending = new Map();
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  const entry = pending.get(message.id);
  if (entry) {
    pending.delete(message.id);
    entry(message);
  }
};
await new Promise((resolve) => (ws.onopen = resolve));
const send = (method, params) =>
  new Promise((resolve) => {
    const next = (id += 1);
    pending.set(next, resolve);
    ws.send(JSON.stringify({ id: next, method, params }));
  });
const evaluate = async (expression) => {
  const response = await send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  return response.result?.result?.value ?? response.result;
};

console.log('pre:', await evaluate("({fn: typeof setExpanded, ipc: !!window.ipc, cls: document.getElementById('stage').className})"));
console.log('call:', await evaluate('setExpanded(true)'));
await new Promise((resolve) => setTimeout(resolve, 1200));
console.log('post:', await evaluate("({cls: document.getElementById('stage').className})"));
ws.close();
process.exit(0);
