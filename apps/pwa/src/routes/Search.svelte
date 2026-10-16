<!-- Экран «Поиск»: полнотекстовый поиск по заметкам (FTS5 на стороне ядра). -->
<script>
  import SearchBar from '../lib/components/SearchBar.svelte';
  import { runSearch, searchResults } from '../lib/stores/api';
  import { t } from '../lib/i18n/index';
</script>

<h2>{$t('search.title')}</h2>
<SearchBar placeholder={$t('search.placeholder')} onsubmit={runSearch} />

{#if $searchResults.length === 0}
  <p class="muted">{$t('search.empty')}</p>
{:else}
  <ul class="cards">
    {#each $searchResults as note (note.id)}
      <li class="card">
        <strong>{note.title}</strong>
        {#if note.body}
          <p>{note.body}</p>
        {/if}
        {#if note.tags.length}
          <p class="muted">{note.tags.join(', ')}</p>
        {/if}
      </li>
    {/each}
  </ul>
{/if}
