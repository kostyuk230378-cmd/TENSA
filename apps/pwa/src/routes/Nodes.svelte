<!--
  Экран «Узлы»: реестр узлов сети (GET /nodes).
  В MVP-бэкенде маршрут отвечает 501 — экран честно это показывает.
-->
<script>
  import { onMount } from 'svelte';
  import { nodes, refreshNodes } from '../lib/stores/api';
  import { t } from '../lib/i18n/index';

  let notImplemented = false;

  async function load() {
    const ok = await refreshNodes();
    notImplemented = !ok;
  }

  onMount(() => {
    void load();
  });

  function lastSeen(seconds) {
    if (!seconds) {
      return '—';
    }
    try {
      return new Date(seconds * 1000).toLocaleString();
    } catch {
      return String(seconds);
    }
  }
</script>

<h2>{$t('nodes.title')}</h2>

{#if notImplemented}
  <div class="card">
    <p class="muted">{$t('nodes.notImplemented')}</p>
  </div>
{:else if $nodes.length === 0}
  <p class="muted">{$t('nodes.empty')}</p>
{:else}
  <table>
    <thead>
      <tr><th>имя</th><th>роль</th><th>онлайн</th><th>RAM свободно, МБ</th><th>связь</th></tr>
    </thead>
    <tbody>
      {#each $nodes as node (node.node_id)}
        <tr>
          <td>{node.name}</td>
          <td>{node.role ?? '—'}</td>
          <td class={node.online ? 'ok-text' : 'muted'}>{node.online ? 'да' : 'нет'}</td>
          <td>{node.ram_available_mb ?? '—'}</td>
          <td class="muted">{lastSeen(node.last_seen)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<button on:click={load}>{$t('nodes.refresh')}</button>
