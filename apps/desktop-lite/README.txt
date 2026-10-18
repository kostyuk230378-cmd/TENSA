Tensa 0.1.0 — установщик для Windows
====================================

Что устанавливается:
  - tensa-server.exe  — HTTP-ядро для PWA (порт 8765) и API одобрений/заметок
  - tensa-cli.exe     — CLI управления ядром (init/status/notes/...)
  - tensa-bot.exe     — Telegram-бот секретаря (long polling)
  - pwa\static\       — собранная PWA (её раздаёт сервер; экран «Чат» с OpenRouter)
  - Tensa.cmd         — ярлык «Tensa»: старт ядра + браузер

Как пользоваться:
  1. Запустите ярлык «Tensa» — откроется ОТДЕЛЬНОЕ ОКНО приложения
     (tensa-desktop.exe: сам поднимает ядро и при закрытии останавливает его).
     Консольный режим — ярлык «Tensa (core console)».
  2. На этом ПК вход без токена: приложение и браузер (127.0.0.1) пускаются
     сразу. Токен нужен только телефону: файл %USERPROFILE%\.tensa\pwa_token.
  3. Телефон: тот же Wi-Fi, откройте http://<ip-пк>:8765/app или установите
     APK: http://<ip-пк>:8765/static/Tensa.apk
     Сервер слушает всю локальную сеть (0.0.0.0:8765). При первом запуске
     Windows спросит про доступ — разрешите tensa-server.exe для ЧАСТНОЙ сети
     (иначе телефон не подключится). В APK укажите адрес ядра и токен.
  4. Telegram-бот: настройка в «Настройках» (токен @BotFather и chat_id);
     затем ярлык «Tensa (Telegram bot)».
     Сеть через прокси: перед запуском "set HTTPS_PROXY=http://127.0.0.1:10809".

Модели:
  - Эмбеддер памяти intfloat/multilingual-e5-small ВХОДИТ В УСТАНОВЩИК:
    файлы лежат в каталоге установки models\multilingual-e5-small и при
    первом запуске ярлыка «Tensa» копируются в
    %USERPROFILE%\.tensa\models\multilingual-e5-small (сеть не нужна).
    Модель используется командами `tensa-cli memory index/search/list`
    (семантический поиск по файлам) — проверено на реальной модели.
  - ПОИСК ПО ФАЙЛАМ: индексируются ВСЕ локальные диски (кроме системных каталогов:
    Windows, Program Files, ProgramData, AppData и т. п.). Первый проход может занять
    часы — он идёт в фоне и не мешает работе; повторные — только изменённые файлы.
    Свои каталоги: TENSA_INDEX_DIRS=C:\Docs;D:\Договоры или [documents] folders.
  - ЛОКАЛЬНАЯ модель (проектная): gemma-4-E2B-it-XS-TS-Q3_K_S.gguf (~1.73 ГБ,
    TheStageAI/gemma-4-E2B-it-GGUF — самый лёгкий квант, XS-TS-Q3_K_S) в
    %USERPROFILE%\.tensa\models. Скачивается одной кнопкой в «Настройках»
    (SHA256 проверяется автоматически); чат работает без интернета и ключей.
  - БОЛЬШИЕ модели (облако, опционально): ключ OpenRouter в
    %USERPROFILE%\.tensa\secrets.toml → [openrouter] credential = "sk-or-…"
    (или https://openrouter.ai/keys). Ядро само выберет облако, если ключ задан.
  - Распознавание речи: модели скачиваются в «Настройках» (в установщике их нет).

Телефон:
  APK собирается из apps/android-lite (в assets включена та же модель e5-small)
  и публикуется в GitHub Releases; установленный APK может быть скопирован
  в pwa\static\Tensa.apk — тогда сервер отдаёт его по /static/Tensa.apk.

Секреты хранятся только в %USERPROFILE%\.tensa (secrets.toml) и не входят
в установщик.

Удаление: «Uninstall Tensa» в меню Пуск (данные в %USERPROFILE%\.tensa
не удаляются — это ваши секреты, заметки и модели).
