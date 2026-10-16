<!--
  Экран «Настройки»: доступ к API, язык и список будущих экранов
  (честные заглушки с указанием этапа).
-->
<script>
  import SettingsPanel from '../lib/components/Settings.svelte';
  import Stub from '../lib/components/Stub.svelte';
  import VoiceButton from '../lib/components/VoiceButton.svelte';
  import { t } from '../lib/i18n/index';

  const FUTURE_KEYS = [
    'stub.documents',
    'stub.email',
    'stub.calendar',
    'stub.apps',
    'stub.voice',
  ];
</script>

<h2>{$t('settings.title')}</h2>
<SettingsPanel />

<div class="card">
  <h3>{$t('settings.future')}</h3>
  <div class="row">
    <VoiceButton />
  </div>
</div>

{#each FUTURE_KEYS as key (key)}
  <Stub title={$t(key)} />
{/each}
