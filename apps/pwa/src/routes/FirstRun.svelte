<!--
  Первый запуск PWA: ввод токена доступа (ТЗ apps/pwa).
  Токен создаётся ядром при первом запуске и показывается в UI десктопа
  (файл pwa_token); после ввода интерфейс переключается на рабочие экраны,
  а корректность токена проверяют запросы (401 → баннер с пояснением).
-->
<script>
  import { setToken } from '../lib/stores/token';
  import { t } from '../lib/i18n/index';

  let value = '';

  function connect() {
    const token = value.trim();
    if (token) {
      setToken(token);
    }
  }
</script>

<div class="card">
  <h2>{$t('firstrun.title')}</h2>
  <p class="muted">{$t('firstrun.hint')}</p>
  <div class="field">
    <label for="firstrun-token">{$t('firstrun.token')}</label>
    <input id="firstrun-token" bind:value autocomplete="off" spellcheck="false" />
  </div>
  <button class="primary" on:click={connect} disabled={!value.trim()}>{$t('firstrun.connect')}</button>
</div>
