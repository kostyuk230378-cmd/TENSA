<!-- Экран «Заметки»: список, создание и удаление (Контракт 3). -->
<script>
  import { createNote, deleteNote, notes, refreshNotes } from '../lib/stores/api';
  import { reportError } from '../lib/stores/ui';
  import { t } from '../lib/i18n/index';

  let title = '';
  let body = '';
  let tags = '';

  async function submit() {
    if (!title.trim()) {
      return;
    }
    try {
      await createNote({
        title: title.trim(),
        body,
        tags: tags
          .split(',')
          .map((tag) => tag.trim())
          .filter((tag) => tag.length > 0),
      });
      title = '';
      body = '';
      tags = '';
    } catch (value) {
      reportError(value);
    }
  }

  async function remove(id) {
    try {
      await deleteNote(id);
    } catch (value) {
      reportError(value);
    }
  }
</script>

<h2>{$t('notes.title')}</h2>

<div class="card">
  <h3>{$t('notes.new')}</h3>
  <div class="field">
    <label for="note-title">{$t('notes.titleField')}</label>
    <input id="note-title" bind:value={title} />
  </div>
  <div class="field">
    <label for="note-body">{$t('notes.bodyField')}</label>
    <textarea id="note-body" rows="3" bind:value={body}></textarea>
  </div>
  <div class="field">
    <label for="note-tags">{$t('notes.tagsField')}</label>
    <input id="note-tags" bind:value={tags} />
  </div>
  <button class="primary" on:click={submit} disabled={!title.trim()}>{$t('notes.save')}</button>
</div>

<button on:click={refreshNotes}>{$t('common.refresh')}</button>

{#if $notes.length === 0}
  <p class="muted">{$t('notes.empty')}</p>
{:else}
  <ul class="cards">
    {#each $notes as note (note.id)}
      <li class="card">
        <div class="row spread">
          <strong>{note.title}</strong>
          <button on:click={() => remove(note.id)}>{$t('notes.delete')}</button>
        </div>
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
