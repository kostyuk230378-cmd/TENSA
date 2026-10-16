<!-- Экран «Статус»: сводное здоровье ядра и компоненты (GET /health). -->
<script>
  import { health, refreshHealth } from '../lib/stores/api';
  import { t } from '../lib/i18n/index';

  $: components = $health?.components ?? {};
  $: componentNames = Object.keys(components).sort();
</script>

<h2>{$t('status.title')}</h2>

<div class="card">
  {#if $health}
    <div class="row spread">
      <span>
        {$t('status.overall')}:
        <strong class={$health.healthy ? 'ok-text' : 'bad-text'}>
          {$health.healthy ? $t('status.healthy') : $t('status.degraded')}
        </strong>
      </span>
      <span class="muted">{$t('status.uptime')}: {$health.uptime}</span>
      <span class="muted">{$t('status.version')}: {$health.version}</span>
    </div>
    {#if $health.degraded_reason}
      <p class="muted">{$health.degraded_reason}</p>
    {/if}
  {:else}
    <p class="muted">—</p>
  {/if}
  <button on:click={refreshHealth}>{$t('status.refresh')}</button>
</div>

{#if componentNames.length}
  <div class="card">
    <h3>{$t('status.components')}</h3>
    <table>
      <thead>
        <tr><th>компонент</th><th>статус</th><th>детали</th></tr>
      </thead>
      <tbody>
        {#each componentNames as name (name)}
          <tr>
            <td>{name}</td>
            <td class={components[name].healthy ? 'ok-text' : 'bad-text'}>
              {components[name].healthy ? 'ok' : 'fail'}
            </td>
            <td class="muted">{components[name].detail ?? ''}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
