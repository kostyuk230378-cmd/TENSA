<!--
  Экран «Чат»: большие модели через OpenRouter напрямую из браузера.

  Ключ и модель хранятся на этом устройстве (localStorage), на сервер
  не отправляются. Запрос идёт с телефона/ПК к openrouter.ai — CORS
  у OpenRouter разрешён для браузерных клиентов.

  [derived] Локальная модель через ядро (/api/v1/query) появится отдельным
  этапом — здесь только облачный контур.
-->
<script>
  import { t } from '../lib/i18n/index';

  /** Ключи localStorage. */
  const KEY_STORE = 'tensa.chat.key';
  const MODEL_STORE = 'tensa.chat.model';

  /** Прочитать значение с защитой от приватного режима. */
  function read(key, fallback) {
    try {
      return localStorage.getItem(key) ?? fallback;
    } catch {
      return fallback;
    }
  }

  /** Сохранить значение. */
  function write(key, value) {
    try {
      if (value) {
        localStorage.setItem(key, value);
      } else {
        localStorage.removeItem(key);
      }
    } catch {
      // приватный режим — настройки не переживут перезапуск
    }
  }

  let apiKey = read(KEY_STORE, '');
  let model = read(MODEL_STORE, 'openai/gpt-4o-mini');
  let input = '';
  let busy = false;
  /** Сообщения: { role: 'user' | 'assistant' | 'error', text }. */
  let messages = [];

  $: apiKey && write(KEY_STORE, apiKey);
  $: model && write(MODEL_STORE, model);

  /** Отправить сообщение в OpenRouter. */
  async function send() {
    const text = input.trim();
    if (!text || busy) {
      return;
    }
    if (!apiKey.trim()) {
      messages = [...messages, { role: 'error', text: $t('chat.noKey') }];
      return;
    }

    messages = [...messages, { role: 'user', text }];
    input = '';
    busy = true;

    try {
      const response = await fetch('https://openrouter.ai/api/v1/chat/completions', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${apiKey.trim()}`,
        },
        body: JSON.stringify({
          model: model.trim() || 'openai/gpt-4o-mini',
          messages: [
            {
              role: 'system',
              content:
                'Ты — Tensa, личный секретарь пользователя. Отвечай кратко и по делу, ' +
                'на языке пользователя. Если нужно действие (письмо, заметка, событие) — ' +
                'предложи его и параметры.',
            },
            { role: 'user', content: text },
          ],
        }),
      });

      if (response.status === 401) {
        throw new Error($t('chat.badKey'));
      }
      if (!response.ok) {
        throw new Error(`OpenRouter HTTP ${response.status}`);
      }

      const json = await response.json();
      const answer = json?.choices?.[0]?.message?.content ?? '';
      messages = [
        ...messages,
        { role: 'assistant', text: answer || '—' },
      ];
    } catch (error) {
      messages = [
        ...messages,
        { role: 'error', text: `${$t('chat.error')}: ${error.message ?? error}` },
      ];
    } finally {
      busy = false;
    }
  }

  /** Enter отправляет, Shift+Enter — перенос строки. */
  function onKeydown(event) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }
</script>

<h2>{$t('chat.title')}</h2>

<div class="card">
  <div class="row">
    <label class="field grow">
      <span>{$t('chat.model')}</span>
      <input bind:value={model} placeholder="openai/gpt-4o-mini" />
    </label>
    <label class="field grow">
      <span>{$t('chat.key')}</span>
      <input type="password" bind:value={apiKey} placeholder="sk-or-…" autocomplete="off" />
    </label>
  </div>
  <p class="muted small">{$t('chat.hint')}</p>
</div>

<div class="card chat">
  {#if messages.length === 0}
    <p class="muted">{$t('chat.empty')}</p>
  {/if}
  {#each messages as message, index (index)}
    <div class="bubble {message.role}">
      <span class="who">
        {message.role === 'user' ? $t('chat.you') : message.role === 'assistant' ? 'Tensa' : '!'}
      </span>
      <p>{message.text}</p>
    </div>
  {/each}
  {#if busy}
    <p class="muted">{$t('chat.thinking')}</p>
  {/if}
</div>

<div class="card">
  <div class="row">
    <textarea
      class="grow"
      rows="2"
      bind:value={input}
      placeholder={$t('chat.placeholder')}
      on:keydown={onKeydown}
    ></textarea>
    <button on:click={send} disabled={busy}>{$t('chat.send')}</button>
  </div>
</div>
