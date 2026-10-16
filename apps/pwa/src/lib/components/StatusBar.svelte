<!-- Статусная строка: состояние SSE-потока и здоровье ядра. -->
<script>
  import { health, streamState } from '../stores/api';

  const STATE_LABELS = {
    connecting: 'подключение…',
    open: 'в сети',
    reconnecting: 'переподключение…',
    unauthorized: 'нет доступа',
    error: 'ошибка потока',
    closed: 'не подключено',
  };

  $: stateLabel = STATE_LABELS[$streamState] ?? $streamState;
  $: hasHealth = $health !== null;
  $: healthy = $health?.healthy === true;
</script>

<span class="statusbar" title="Поток событий ядра (SSE) и здоровье">
  <span class="dot" class:ok={healthy} class:bad={hasHealth && !healthy}></span>
  {stateLabel}
  {#if hasHealth}
    <em>v{$health.version}</em>
  {/if}
</span>
