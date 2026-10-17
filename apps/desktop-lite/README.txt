Tensa 0.1.0 — установщик для Windows
====================================

Что устанавливается:
  - tensa-server.exe  — HTTP-ядро для PWA (порт 8765) и API одобрений/заметок
  - tensa-cli.exe     — CLI управления ядром (init/status/notes/...)
  - tensa-bot.exe     — Telegram-бот секретаря (long polling)
  - pwa\static\       — собранная PWA (её раздаёт сервер)
  - Tensa.cmd         — ярлык «Tensa» на рабочем столе: старт ядра + браузер

Как пользоваться:
  1. Запустите ярлык «Tensa» — откроется браузер на http://127.0.0.1:8765/app
     (окно консоли держит сервер; закрыли окно — сервер остановлен).
  2. Токен доступа — файл %USERPROFILE%\.tensa\pwa_token (ядро создаёт его
     при первом запуске). Вставьте токен на экране подключения PWA.
  3. Телефон: тот же Wi-Fi, откройте http://<ip-пк>:8765/app или установите
     APK: http://<ip-пк>:8765/static/Tensa.apk
  4. Telegram-бот: ярлык «Tensa (Telegram bot)»; токен бота и чат владельца —
     в %USERPROFILE%\.tensa\secrets.toml (или env TELEGRAM_BOT_TOKEN).
     Сеть через прокси: перед запуском "set HTTPS_PROXY=http://127.0.0.1:10809".

Секреты хранятся только в %USERPROFILE%\.tensa (secrets.toml) и не входят
в установщик.

Удаление: «Удалить Tensa» в меню Пуск (данные в %USERPROFILE%\.tensa
не удаляются — это ваши секреты и заметки).
