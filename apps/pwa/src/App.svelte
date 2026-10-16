<!--
  Оболочка PWA Tensa (ТЗ apps/pwa): навигация по экранам, статусная строка,
  баннер ошибок и жизненный цикл SSE-потока.

  Пока токен не введён — показывается экран подключения (FirstRun);
  после ввода загружаются данные и открывается поток событий.
-->
<script>
  import { onDestroy, onMount } from 'svelte';
  import { currentToken, token } from './lib/stores/token';
  import { clearError, error, screen } from './lib/stores/ui';
  import { boot, stopEventStream } from './lib/stores/api';
  import { t } from './lib/i18n/index';

  import StatusBar from './lib/components/StatusBar.svelte';
  import FirstRun from './routes/FirstRun.svelte';
  import Status from './routes/Status.svelte';
  import Approvals from './routes/Approvals.svelte';
  import Notes from './routes/Notes.svelte';
  import Search from './routes/Search.svelte';
  import Nodes from './routes/Nodes.svelte';
  import Settings from './routes/Settings.svelte';

  /** Навигация: id экрана ←→ ключ перевода. */
  const NAV = [
    { id: 'status', key: 'nav.status' },
    { id: 'approvals', key: 'nav.approvals' },
    { id: 'notes', key: 'nav.notes' },
    { id: 'search', key: 'nav.search' },
    { id: 'nodes', key: 'nav.nodes' },
    { id: 'settings', key: 'nav.settings' },
  ];

  let booted = false;

  $: authorized = $token.length > 0;

  // Появление доступа запускает загрузку; потеря — закрывает поток.
  $: if (authorized && !booted) {
    booted = true;
    boot();
  }
  $: if (!authorized && booted) {
    booted = false;
    stopEventStream();
  }

  onMount(() => {
    if (currentToken()) {
      booted = true;
      boot();
    }
  });

  onDestroy(() => stopEventStream());
</script>

<div class="shell">
  {#if !authorized}
    <FirstRun />
  {:else}
    <header class="top">
      <span class="brand">Tensa <small>{$t('app.tagline')}</small></span>
      <StatusBar />
    </header>
    <nav>
      {#each NAV as item (item.id)}
        <button
          class:active={$screen === item.id}
          on:click={() => screen.set(item.id)}>{$t(item.key)}</button>
      {/each}
    </nav>

    {#if $error}
      <div class="banner" role="alert">
        <span>{$error}</span>
        <button on:click={clearError} aria-label="закрыть">×</button>
      </div>
    {/if}

    <main>
      {#if $screen === 'status'}
        <Status />
      {:else if $screen === 'approvals'}
        <Approvals />
      {:else if $screen === 'notes'}
        <Notes />
      {:else if $screen === 'search'}
        <Search />
      {:else if $screen === 'nodes'}
        <Nodes />
      {:else if $screen === 'settings'}
        <Settings />
      {/if}
    </main>
  {/if}
</div>
