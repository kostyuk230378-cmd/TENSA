<!--
  Экран «Настройки»: доступ к API, язык и список будущих экранов
  (честные заглушки с указанием этапа).
-->
<script>
  import { onMount } from 'svelte';
  import { client } from '../lib/stores/api';
  import SettingsPanel from '../lib/components/Settings.svelte';
  import Stub from '../lib/components/Stub.svelte';
  import VoiceButton from '../lib/components/VoiceButton.svelte';
  import { t } from '../lib/i18n/index';

  /** Статус индекса файлов. */
  let filesStatus = null;
  /** Ошибка индекса. */
  let filesError = '';
  /** Кнопка занята. */
  let filesBusy = false;

  /** Загрузить статус индекса. */
  async function loadFiles() {
    try {
      filesStatus = await client.filesStatus();
      filesError = '';
    } catch (error) {
      filesError = error.message ?? String(error);
    }
  }

  /** Запустить переиндексацию и обновить статус. */
  async function reindexFiles() {
    filesBusy = true;
    try {
      await client.filesIndex();
      await loadFiles();
    } catch (error) {
      filesError = error.message ?? String(error);
    } finally {
      filesBusy = false;
    }
  }

  onMount(loadFiles);

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
  <h3>{$t('settings.files')}</h3>
  {#if filesStatus}
    <p class="muted">
      {$t('settings.filesIndexed')}: {filesStatus.files} · {$t('settings.filesScanned')}:
      {filesStatus.scanned} · {$t('settings.filesUpdated')}: {filesStatus.indexed_run}
      {#if filesStatus.scanning}· {$t('settings.filesScanning')}{/if}
    </p>
    <p class="muted small">{$t('settings.filesDirs')}: {filesStatus.dirs.join(' · ') || '—'}</p>
    {#if filesStatus.current_dir}
      <p class="muted small">{filesStatus.current_dir}</p>
    {/if}
  {:else}
    <p class="muted">—</p>
  {/if}
  {#if filesError}
    <p class="bad-text">{filesError}</p>
  {/if}
  <button on:click={reindexFiles} disabled={filesBusy}>{$t('settings.filesReindex')}</button>
</div>

<div class="card">
  <h3>{$t('settings.future')}</h3>
  <div class="row">
    <VoiceButton />
  </div>
</div>

{#each FUTURE_KEYS as key (key)}
  <Stub title={$t(key)} />
{/each}
