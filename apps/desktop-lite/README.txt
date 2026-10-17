Tensa 0.1.0 — установщик для Windows
====================================

Что устанавливается:
  - tensa-server.exe  — HTTP-ядро для PWA (порт 8765) и API одобрений/заметок
  - tensa-cli.exe     — CLI управления ядром (init/status/notes/...)
  - tensa-bot.exe     — Telegram-бот секретаря (long polling)
  - pwa\static\       — собранная PWA (её раздаёт сервер; экран «Чат» с OpenRouter)
  - Tensa.cmd         — ярлык «Tensa»: старт ядра + браузер
  - download_models.ps1 / DownloadModels.cmd — загрузчик моделей

Как пользоваться:
  1. Запустите ярлык «Tensa» — откроется браузер на http://127.0.0.1:8765/app
     (окно консоли держит сервер; закрыли окно — сервер остановлен).
  2. Токен доступа — файл %USERPROFILE%\.tensa\pwa_token (ядро создаёт его
     при первом запуске). Вставьте токен на экране подключения PWA.
  3. Телефон: тот же Wi-Fi, откройте http://<ip-пк>:8765/app или установите
     APK: http://<ip-пк>:8765/static/Tensa.apk
     Сервер слушает всю локальную сеть (0.0.0.0:8765). При первом запуске
     Windows спросит про доступ — разрешите tensa-server.exe для ЧАСТНОЙ сети
     (иначе телефон не подключится). В APK укажите адрес ядра и токен.
  4. Telegram-бот: ярлык «Tensa (Telegram bot)»; токен бота и чат владельца —
     в %USERPROFILE%\.tensa\secrets.toml (или env TELEGRAM_BOT_TOKEN).
     Сеть через прокси: перед запуском "set HTTPS_PROXY=http://127.0.0.1:10809".

Модели:
  - Эмбеддер памяти intfloat/multilingual-e5-small ВХОДИТ В УСТАНОВЩИК:
    файлы лежат в каталоге установки models\multilingual-e5-small и при
    первом запуске ярлыка «Tensa» копируются в
    %USERPROFILE%\.tensa\models\multilingual-e5-small (сеть не нужна).
    Модель используется командами `tensa-cli memory index/search/list`
    (семантический поиск по файлам) — проверено на реальной модели.
  - Большие модели (облако): в PWA — экран «Чат» (OpenRouter) и в APK — «Чат».
    Нужен ключ OpenRouter: https://openrouter.ai/keys (вводится в приложении,
    хранится только на устройстве).
  - Локальная маленькая LLM (GGUF) для ядра — отдельная от памяти модель:
    каталог %USERPROFILE%\.tensa\models; ярлык «Tensa (download models)»
    кладёт google_gemma-4-E2B-it-Q3_K_S.gguf (~3.1 ГБ) или положите свой .gguf.
  - Распознавание речи: DownloadModels.cmd -Whisper (ggml-base.bin, ~142 МБ).

Телефон:
  APK собирается из apps/android-lite (в assets включена та же модель e5-small)
  и публикуется в GitHub Releases; установленный APK может быть скопирован
  в pwa\static\Tensa.apk — тогда сервер отдаёт его по /static/Tensa.apk.

Секреты хранятся только в %USERPROFILE%\.tensa (secrets.toml) и не входят
в установщик.

Удаление: «Uninstall Tensa» в меню Пуск (данные в %USERPROFILE%\.tensa
не удаляются — это ваши секреты, заметки и модели).
