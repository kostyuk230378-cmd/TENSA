/** Выполнить JS в WebView2 (проверка drag move). */
const expression = process.argv[2] ?? "sendIpc('move 70 25')";
const list = await (await fetch('http://127.0.0.1:9222/json/list')).json();
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
const response = await new Promise((resolve) => {
  const next = (id += 1);
  pending.set(next, resolve);
  ws.send(JSON.stringify({ id: next, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
});
console.log(JSON.stringify(response.result?.result?.value ?? response.result));
ws.close();
process.exit(0);
