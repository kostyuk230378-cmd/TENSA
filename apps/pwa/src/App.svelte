<!--
  Оболочка приложения Tensa: человеческий, «чат-первый» интерфейс.

  Главный экран — чат секретаря. Внизу три кнопки: Чат, Одобрения (со счётчиком
  ожидающих) и Меню — всё остальное (заметки, поиск, календарь, почта, узлы,
  статус, настройки) прячется в меню, а не висит рядом из семи вкладок.

  Доступ: на этом ПК — без токена (loopback), с телефона — по Bearer-токену.
-->
<script>
  import { onDestroy, onMount } from 'svelte';
  import { currentToken, token } from './lib/stores/token';
  import { approvals, boot, client, stopEventStream } from './lib/stores/api';
  import { clearError, error, localAccess, screen } from './lib/stores/ui';
  import { t } from './lib/i18n/index';

  import StatusBar from './lib/components/StatusBar.svelte';
  import FirstRun from './routes/FirstRun.svelte';
  import Chat from './routes/Chat.svelte';
  import Approvals from './routes/Approvals.svelte';
  import Notes from './routes/Notes.svelte';
  import Search from './routes/Search.svelte';
  import Nodes from './routes/Nodes.svelte';
  import Settings from './routes/Settings.svelte';
  import Status from './routes/Status.svelte';

  /** Разделы, доступные из меню (не на нижней панели). */
  const MENU = [
    { id: 'notes', key: 'nav.notes' },
    { id: 'search', key: 'nav.search' },
    { id: 'status', key: 'nav.status' },
    { id: 'nodes', key: 'nav.nodes' },
    { id: 'settings', key: 'nav.settings' },
  ];

  let booted = false;
  /** Открыто ли меню-лист. */
  let menuOpen = false;

  $: authorized = $token.length > 0 || $localAccess;
  $: pending = $approvals.filter((item) => item.status === 'pending').length;

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
      return;
    }
    // Локальный вход без токена: сервер на этом ПК пускает loopback-запросы.
    client
      .health()
      .then(() => {
        localAccess.set(true);
      })
      .catch(() => {
        // Удалённый вход без токена — показываем экран подключения.
      });
  });

  onDestroy(() => stopEventStream());

  /** Перейти на экран из меню. */
  function go(id) {
    screen.set(id);
    menuOpen = false;
  }
</script>

<div class="shell">
  {#if !authorized}
    <FirstRun />
  {:else}
    <header class="top">
      <span class="brand">Tensa</span>
      <StatusBar />
    </header>

    {#if $error}
      <div class="banner" role="alert">
        <span>{$error}</span>
        <button on:click={clearError} aria-label="закрыть">×</button>
      </div>
    {/if}

    <main>
      {#if $screen === 'chat'}
        <Chat />
      {:else if $screen === 'approvals'}
        <Approvals />
      {:else if $screen === 'notes'}
        <Notes />
      {:else if $screen === 'search'}
        <Search />
      {:else if $screen === 'status'}
        <Status />
      {:else if $screen === 'nodes'}
        <Nodes />
      {:else if $screen === 'settings'}
        <Settings />
      {/if}
    </main>

    {#if menuOpen}
      <div class="sheet" role="dialog" aria-label={$t('nav.menu')}>
        <div class="sheet-head">
          <strong>{$t('nav.menu')}</strong>
          <button on:click={() => (menuOpen = false)} aria-label="закрыть">×</button>
        </div>
        {#each MENU as item (item.id)}
          <button class="sheet-item" on:click={() => go(item.id)}>{$t(item.key)}</button>
        {/each}
      </div>
      <button class="scrim" on:click={() => (menuOpen = false)} aria-label="закрыть меню"></button>
    {/if}

    <nav class="bottom">
      <button class:active={$screen === 'chat'} on:click={() => go('chat')}>
        {$t('nav.chat')}
      </button>
      <button class:active={$screen === 'approvals'} on:click={() => go('approvals')}>
        {$t('nav.approvals')}
        {#if pending > 0}<span class="badge">{pending}</span>{/if}
      </button>
      <button class:active={menuOpen} on:click={() => (menuOpen = !menuOpen)}>
        {$t('nav.menu')}
      </button>
    </nav>
  {/if}
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    height: 100dvh;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .brand {
    font-weight: 700;
    letter-spacing: 0.3px;
  }

  main {
    flex: 1;
    overflow-y: auto;
    padding: 12px 14px 8px;
  }

  .bottom {
    display: flex;
    gap: 6px;
    padding: 8px 10px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .bottom button {
    flex: 1;
    position: relative;
    padding: 12px 8px;
    font-size: 15px;
    border-radius: 12px;
  }

  .bottom button.active {
    background: rgba(96, 165, 250, 0.18);
    border-color: rgba(96, 165, 250, 0.5);
  }

  .badge {
    position: absolute;
    top: 4px;
    right: 10px;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    background: #ef4444;
    color: #fff;
    font-size: 11px;
    line-height: 18px;
    text-align: center;
  }

  .sheet {
    position: fixed;
    left: 12px;
    right: 12px;
    bottom: 64px;
    z-index: 30;
    padding: 10px;
    border-radius: 16px;
    background: #131a24;
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
  }

  .sheet-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px 8px;
  }

  .sheet-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 12px 10px;
    margin: 2px 0;
    border-radius: 10px;
    background: transparent;
    border: 0;
    font-size: 15px;
  }

  .sheet-item:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    background: rgba(0, 0, 0, 0.35);
    border: 0;
  }
</style>
