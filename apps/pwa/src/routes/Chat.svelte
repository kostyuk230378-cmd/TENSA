<!--
  Экран «Чат»: сначала ядро Tensa (`POST /api/v1/query`) — оно само выбирает
  облачную модель (ключ OpenRouter в secrets.toml) или локальную GGUF.
  Если ядро не настроено (503) — резервный прямой вызов OpenRouter с ключом
  из полей ниже (ключ остаётся на устройстве).

  [derived] Прямой вызов — страховка для установки без ключа в secrets;
  основной контур — через ядро (ключ не покидает ПК).
-->
<script>
  import { client } from '../lib/stores/api';
  import { t } from '../lib/i18n/index';

  /** Ключи localStorage (резервный прямой доступ). */
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
  /** Сообщения: { role: 'user' | 'assistant' | 'error' | 'system', text }. */
  let messages = [];

  $: apiKey && write(KEY_STORE, apiKey);
  $: model && write(MODEL_STORE, model);

  /** Добавить сообщение и прокрутить. */
  function push(role, text) {
    messages = [...messages, { role, text }];
  }

  /** Отправить сообщение: ядро → (при 503/501 и наличии ключа) прямой OpenRouter. */
  async function send() {
    const text = input.trim();
    if (!text || busy) {
      return;
    }

    push('user', text);
    input = '';
    busy = true;

    try {
      const reply = await client.query(text);
      const suffix = reply.degraded ? ` ${$t('chat.localFallback')}` : '';
      push('assistant', `${reply.text}${suffix}`);
    } catch (error) {
      const coreUnavailable = error?.status === 503 || error?.status === 501;
      if (coreUnavailable && apiKey.trim()) {
        try {
          push('assistant', await directOpenRouter(text));
        } catch (directError) {
          push('error', `${$t('chat.error')}: ${directError.message ?? directError}`);
        }
      } else if (coreUnavailable) {
        push('error', $t('chat.coreNotConfigured'));
      } else {
        push('error', `${$t('chat.error')}: ${error.message ?? error}`);
      }
    } finally {
      busy = false;
    }
  }

  /** Резервный прямой запрос к OpenRouter с ключом из настроек экрана. */
  async function directOpenRouter(text) {
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
    return json?.choices?.[0]?.message?.content || '—';
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
