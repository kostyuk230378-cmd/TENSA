<!-- Очередь одобрения: карточки действий и кнопки решения (Контракт 1). -->
<script>
  import { approvals, decide } from '../stores/api';
  import { reportError } from '../stores/ui';
  import { t } from '../i18n/index';

  async function act(id, approve) {
    try {
      await decide(id, approve);
    } catch (value) {
      reportError(value);
    }
  }

  function payloadText(payload) {
    try {
      return JSON.stringify(payload, null, 2);
    } catch {
      return String(payload);
    }
  }
</script>

{#if $approvals.length === 0}
  <p class="muted">{$t('approvals.empty')}</p>
{:else}
  <ul class="cards">
    {#each $approvals as action (action.id)}
      <li class="card">
        <div class="row spread">
          <strong>{action.kind}</strong>
          <span class="muted">{action.status}</span>
        </div>
        <pre>{payloadText(action.payload)}</pre>
        <div class="row">
          <button class="primary" on:click={() => act(action.id, true)}>{$t('approvals.approve')}</button>
          <button on:click={() => act(action.id, false)}>{$t('approvals.reject')}</button>
        </div>
      </li>
    {/each}
  </ul>
{/if}
