<!-- Строка поиска: значение отдаётся наружу колбэком onsubmit. -->
<script>
  export let placeholder = 'Поиск';
  export let onsubmit = () => {};

  let value = '';

  function submit() {
    const query = value.trim();
    if (query) {
      onsubmit(query);
    }
  }
</script>

<form class="searchbar" on:submit|preventDefault={submit}>
  <input bind:value {placeholder} enterkeyhint="search" aria-label={placeholder} />
  <button type="submit" aria-label="найти">→</button>
</form>
