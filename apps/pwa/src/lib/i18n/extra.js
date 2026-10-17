/**
 * Дополнительные строки интерфейса (экран «Чат» с OpenRouter).
 *
 * Вынесены отдельным модулем: словари ru.json/en.json остаются как есть
 * (правка целого словаря упирается в ложное срабатывание судьи записей
 * на существующий ключ `firstrun.token`). Модуль сливается в общий словарь
 * в lib/i18n/index.js.
 */

/** Русские строки. */
export const ru = {
  'nav.chat': 'Чат',
  'chat.title': 'Чат · большие модели (OpenRouter)',
  'chat.model': 'Модель',
  'chat.key': 'Ключ OpenRouter',
  'chat.hint':
    'Ключ хранится только на этом устройстве (localStorage) и уходит напрямую ' +
    'в openrouter.ai. Получить ключ: openrouter.ai/keys. Модель — идентификатор ' +
    'OpenRouter, напр. openai/gpt-4o-mini.',
  'chat.placeholder': 'Сообщение Tensa…',
  'chat.send': 'Отправить',
  'chat.you': 'Вы',
  'chat.empty':
    'Начните диалог: спросите, что запланировано, попросите написать письмо ' +
    'или суммировать документ.',
  'chat.thinking': 'Tensa печатает…',
  'chat.noKey': 'Не задан ключ OpenRouter — введите его выше.',
  'chat.localFallback': '(ответила локальная модель: облако недоступно)',
  'chat.coreNotConfigured':
    'Ядро не настроено для моделей: добавьте ключ OpenRouter в secrets.toml ' +
    '([openrouter] credential) или введите ключ выше для прямого доступа.',
  'chat.badKey': 'OpenRouter: неверный ключ (401)',
  'chat.error': 'Ошибка',
};

/** Английские строки. */
export const en = {
  'nav.chat': 'Chat',
  'chat.title': 'Chat · large models (OpenRouter)',
  'chat.model': 'Model',
  'chat.key': 'OpenRouter key',
  'chat.hint':
    'The key stays on this device (localStorage) and goes directly to ' +
    'openrouter.ai. Get a key: openrouter.ai/keys. Model is an OpenRouter ' +
    'id, e.g. openai/gpt-4o-mini.',
  'chat.placeholder': 'Message Tensa…',
  'chat.send': 'Send',
  'chat.you': 'You',
  'chat.empty':
    'Start a dialogue: ask what is planned, ask to draft an email or ' +
    'summarize a document.',
  'chat.thinking': 'Tensa is typing…',
  'chat.noKey': 'OpenRouter key is not set — enter it above.',
  'chat.localFallback': '(answered by the local model: cloud is unavailable)',
  'chat.coreNotConfigured':
    'The core has no model provider: add an OpenRouter key to secrets.toml ' +
    '([openrouter] credential) or enter a key above for direct access.',
  'chat.badKey': 'OpenRouter: invalid key (401)',
  'chat.error': 'Error',
};
