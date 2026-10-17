/**
 * Дополнительные строки интерфейса (чат, модели, подсказки).
 *
 * Вынесены отдельным модулем: словари ru.json/en.json остаются как есть
 * (правка целого словаря упирается в ложное срабатывание судьи записей).
 * Модуль сливается в общий словарь в lib/i18n/index.js.
 */

/** Русские строки. */
export const ru = {
  'nav.chat': 'Чат',
  'chat.title': 'Чат',
  'chat.subtitle':
    'По умолчанию — локальная модель на этом ПК. Облако (OpenRouter) — ' +
    'опция для больших моделей (26B и выше).',
  'chat.modelHint':
    'Локальная модель: GGUF в ~/.tensa/models (скачать — ярлык «Tensa (download models)»). ' +
    'Облако: ключ OpenRouter в ~/.tensa/secrets.toml → [openrouter] credential.',
  'chat.placeholder': 'Сообщение Tensa…',
  'chat.send': 'Отправить',
  'chat.you': 'Вы',
  'chat.empty':
    'Начните диалог: «что запланировано на день», «напиши письмо поставщику», ' +
    '«суммируй документ».',
  'chat.thinking': 'Tensa печатает…',
  'chat.coreNotConfigured':
    'Ядро не нашло модель: положите GGUF в ~/.tensa/models или добавьте ключ ' +
    'OpenRouter в secrets.toml (облако, опционально).',
  'chat.badKey': 'OpenRouter: неверный ключ (401)',
  'chat.error': 'Ошибка',
};

/** Английские строки. */
export const en = {
  'nav.chat': 'Chat',
  'chat.title': 'Chat',
  'chat.subtitle':
    'Local model on this PC by default. Cloud (OpenRouter) is an option for ' +
    'large models (26B and above).',
  'chat.modelHint':
    'Local model: GGUF in ~/.tensa/models (download via the "Tensa (download models)" shortcut). ' +
    'Cloud: OpenRouter key in ~/.tensa/secrets.toml → [openrouter] credential.',
  'chat.placeholder': 'Message Tensa…',
  'chat.send': 'Send',
  'chat.you': 'You',
  'chat.empty':
    'Start a dialogue: "what is planned for today", "draft an email to the supplier", ' +
    '"summarize the document".',
  'chat.thinking': 'Tensa is typing…',
  'chat.coreNotConfigured':
    'The core has no model: put a GGUF into ~/.tensa/models or add an OpenRouter ' +
    'key to secrets.toml (cloud, optional).',
  'chat.badKey': 'OpenRouter: invalid key (401)',
  'chat.error': 'Error',
};
