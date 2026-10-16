<!-- Экран «Одобрения»: очередь действий и решения (Контракт 1). -->
<script>
  import ApprovalsList from '../lib/components/ApprovalsList.svelte';
  import { refreshApprovals } from '../lib/stores/api';
  import { t } from '../lib/i18n/index';
</script>

<h2>{$t('approvals.title')}</h2>
<button on:click={refreshApprovals}>{$t('common.refresh')}</button>

<ApprovalsList />
