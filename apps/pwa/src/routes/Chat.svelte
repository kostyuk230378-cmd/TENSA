<!--
  Экран «Чат»: локальная модель ядра ищет по всем дискам; найденные файлы
  показываются ссылками-кнопками: «Открыть» (запуск на этом ПК) и «Скачать»
  (забрать на телефон). Обычный путь ответа — POST /api/v1/query.
-->
<script>
  import { client } from '../lib/stores/api';
  import { t } from '../lib/i18n/index';

  let input = '';
  let busy = false;
  /** Сообщения: { role, text, meta, files: [{path,name,snippet}] }. */
  let messages = [];

  /** Добавить сообщение. */
  function push(role, text, meta = '', files = []) {
    messages = [...messages, { role, text, meta, files }];
  }

  /** Отправить сообщение в ядро (локальная или облачная модель + файлы). */
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
      const files = Array.isArray(reply.action_payload?.files)
        ? reply.action_payload.files
        : [];
      push('assistant', reply.text, meta, files);
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

  /** Открыть файл на этом ПК программой по умолчанию. */
  async function openFile(file) {
    try {
      await client.openFile(file.path);
      push('system', `${$t('chat.opened')}: ${file.name}`);
    } catch (error) {
      push('error', `${$t('chat.openFailed')}: ${error.message ?? error}`);
    }
  }

  /** Скачать файл (телефон/браузер). */
  async function downloadFile(file) {
    try {
      const blob = await client.downloadFile(file.path);
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = file.name;
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      push('error', `${$t('chat.openFailed')}: ${error.message ?? error}`);
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
      {#if message.files.length}
        <p class="muted small">{message.text.split('\n')[0]}</p>
      {:else}
        <p>{message.text}</p>
      {/if}
      {#if message.files.length}
        <ul class="files">
          {#each message.files as file (file.path)}
            <li>
              <button class="file-link" on:click={() => openFile(file)}>{file.name}</button>
              <div class="muted small file-path">{file.path}</div>
              <button class="file-download" on:click={() => downloadFile(file)}>
                {$t('chat.download')}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
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

<style>
  .files {
    list-style: none;
    margin: 8px 0 4px;
    padding: 0;
  }

  .files li {
    padding: 8px 0;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .file-link {
    padding: 0;
    border: 0;
    background: transparent;
    color: #7dd3fc;
    font-size: 15px;
    text-align: left;
    text-decoration: underline;
    cursor: pointer;
  }

  .file-path {
    margin: 2px 0 4px;
    word-break: break-all;
  }

  .file-download {
    padding: 4px 10px;
    font-size: 13px;
    border-radius: 8px;
  }
</style>
