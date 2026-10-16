<!--
  Настройки доступа PWA (ТЗ apps/pwa: Settings): адрес API, токен, язык.
  Черновики полей применяются кнопкой «Сохранить» (и перед «Проверить»),
  чтобы смена адреса и токена отрабатывала атомарно.
-->
<script>
  import { client } from '../stores/api';
  import { baseUrl, language, reportError, setBaseUrl, setLanguage } from '../stores/ui';
  import { currentToken, setToken } from '../stores/token';
  import { t } from '../i18n/index';

  let baseDraft = $baseUrl;
  let tokenDraft = currentToken();
  let checkState = '';

  function persistDrafts() {
    setBaseUrl(baseDraft);
    setToken(tokenDraft);
  }

  async function check() {
    persistDrafts();
    checkState = 'checking';
    try {
      const status = await client.health();
      checkState = status.healthy ? 'ok' : 'degraded';
    } catch (value) {
      checkState = '';
      reportError(value);
    }
  }

  $: checkText =
    checkState === 'checking'
      ? $t('firstrun.checking')
      : checkState === 'ok'
        ? $t('firstrun.ok')
        : checkState === 'degraded'
          ? $t('status.degraded')
          : '';
</script>

<div class="card">
  <div class="field">
    <label for="settings-base">{$t('settings.base')}</label>
    <input id="settings-base" bind:value={baseDraft} placeholder="/api/v1" autocomplete="off" />
    <p class="hint">{$t('settings.baseHint')}</p>
  </div>

  <div class="field">
    <label for="settings-token">{$t('settings.token')}</label>
    <input id="settings-token" bind:value={tokenDraft} autocomplete="off" spellcheck="false" />
  </div>

  <div class="field">
    <label for="settings-lang">{$t('settings.language')}</label>
    <select id="settings-lang" value={$language} on:change={(event) => setLanguage(event.currentTarget.value)}>
      <option value="ru">Русский</option>
      <option value="en">English</option>
    </select>
  </div>

  <div class="row">
    <button class="primary" on:click={persistDrafts}>{$t('settings.save')}</button>
    <button on:click={check}>{$t('firstrun.check')}</button>
    {#if checkText}
      <span class="muted">{checkText}</span>
    {/if}
  </div>
</div>
