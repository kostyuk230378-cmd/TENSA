<!--
  Экран «Чат»: по умолчанию — локальная модель ядра (GGUF). Облако OpenRouter
  (большие модели, например 26B) — опция ядра: ключ в ~/.tensa/secrets.toml.
  Ключей в браузере нет: всё идёт через POST /api/v1/query.
-->
<script>
  import { client } from '../lib/stores/api';
  import { t } from '../lib/i18n/index';

  let input = '';
  let busy = false;
  /** Сообщения: { role: 'user' | 'assistant' | 'error' | 'system', text, meta }. */
  let messages = [];

  /** Добавить сообщение. */
  function push(role, text, meta = '') {
    messages = [...messages, { role, text, meta }];
  }

  /** Отправить сообщение в ядро (оно выбирает локальную или облачную модель). */
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
      const meta = `${reply.model_used}${reply.degraded ? ' · локальная' : ''}`;
      push('assistant', reply.text, meta);
    } catch (error) {
      if (error?.status === 503) {
        push('system', $t('chat.coreNotConfigured'));
      } else {
        push('error', `${$t('chat.error')}: ${error.message ?? error}`);
      }
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
<p class="muted">{$t('chat.subtitle')}</p>

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
      {#if message.meta}
        <span class="muted small">{message.meta}</span>
      {/if}
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
  <p class="muted small">{$t('chat.modelHint')}</p>
</div>
