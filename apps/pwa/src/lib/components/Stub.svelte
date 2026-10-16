<!-- Заглушка экрана «ещё нет»: честно называет этап, в котором экран появится. -->
<script>
  export let title = '';
  export let note = '';
</script>

<div class="card">
  <strong>{title}</strong>
  {#if note}
    <p class="muted">{note}</p>
  {/if}
</div>
