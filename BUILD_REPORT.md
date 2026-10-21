# Tensa — отчёт о сборке и публикации

**Дата:** 21.10.2026  
**Репозиторий:** <https://github.com/kostyuk230378-cmd/TENSA>  
**Ветка:** `main`  
**Последний код перед отчётом:** `5f32112` — `feat(android): ASR fallback through PC and tested mobile UI`  
**Последний релиз:** [v0.2.14](https://github.com/kostyuk230378-cmd/TENSA/releases/tag/v0.2.14)  
**CI:** зелёный на `6836b51` ([run 39599324087](https://github.com/kostyuk230378-cmd/TENSA/actions/runs/39599324087)); для Java-коммита `5f32112` — отдельная проверка.

Документ описывает, как собраны Windows-дистрибутив и Android APK, что именно
опубликовано в GitHub и какие проверки выполнены. Это отчёт о сборке, а не
пользовательская инструкция; быстрый старт — в `README.md`.

---

## 1. Что опубликовано

| Артефакт | Размер | SHA-256 | Где |
|---|---:|---|---|
| `TensaSetup.exe` v0.2.13 | 231 119 561 байт (220.4 МиБ) | `614b111029fa7dc24455b94848c170a97e26adffa3428c54f18ed8fccbd3cc82` | GitHub Release v0.2.14 |
| `Tensa.apk` v0.2.14 | 239 146 225 байт (228.1 МиБ) | `e3e93f32c86ea2a232f100a3afb5e373997a1c674ae8290c75abb15442dec223` | GitHub Release v0.2.14 |
| Исходный код | — | — | `main`, commit `5f32112` |
| Отчёт | — | — | `BUILD_REPORT.md` в `main` и в assets релиза |

Ссылки на скачивание:

- <https://github.com/kostyuk230378-cmd/TENSA/releases/download/v0.2.14/TensaSetup.exe>
- <https://github.com/kostyuk230378-cmd/TENSA/releases/download/v0.2.14/Tensa.apk>

Бинарники не коммитятся в git: они публикуются как assets GitHub Release.
В репозитории лежат исходники, скрипты сборки и документация.

---

## 2. Состав дистрибутивов

### Windows Setup

`TensaSetup.exe` устанавливается в `%LOCALAPPDATA%\Tensa` без прав
администратора и содержит:

- `tensa-server.exe` — HTTP-ядро (порт 8765);
- `tensa-desktop.exe` — оконный клиент/виджет;
- `tensa-cli.exe` — CLI;
- `tensa-bot.exe` — Telegram-бот;
- `tensa-whisper.exe` — fallback-helper whisper.cpp;
- `tensa-asr.exe` — постоянный ASR-worker (Whisper, Parakeet, Canary, Nemotron);
- `pwa/static` — PWA и виджет;
- `models/multilingual-e5-small` — эмбеддер памяти e5-small;
- `tensa.ico` — иконка TensorLogix;
- ярлыки, `Tensa.cmd`, `TensaBot.cmd`, деинсталлятор.

LLM Gemma и модели распознавания речи **не входят** в Setup. Они скачиваются
из интернета в каталог установки `Tensa\models` через настройки приложения.
OpenRouter-ключ и токены не входят в дистрибутив.

### Android APK

`Tensa.apk` — нативный Java-клиент без Gradle и сторонних библиотек:

- первый запуск: сервер + токен ядра;
- dashboard состояния связи;
- чат через ядро Tensa, резервный прямой OpenRouter;
- голосовой ввод через системный `SpeechRecognizer` Android;
- поиск файлов на ПК и скачивание в Downloads;
- одобрения, заметки, почта, календарь, настройки;
- тёмная тема TensorLogix и логотип.

APK содержит `assets/multilingual-e5-small` (эмбеддер памяти, как и раньше).
LLM и ASR-модели на телефоне не запускаются: телефон — пульт к ПК.
APK подписан debug-ключом демо-сборки; для production нужен отдельный
release keystore.

---

## 3. Архитектура сборки

```
Rust workspace
├─ tensa-server      HTTP API + PWA + desktop backend
├─ tensa-desktop     окно/виджет Windows
├─ tensa-asr         worker: Whisper/Parakeet/Canary/Nemotron
├─ tensa-whisper     fallback whisper.cpp
├─ tensa-cli         CLI
├─ tensa-tools       SMTP/Telegram/документы
└─ ...
        │
        ├─ NSIS ───────────────► TensaSetup.exe
        │
Android Java (apps/android-lite)
        └─ aapt2 + javac + d8 + apksigner ─► Tensa.apk
```

### Поток данных

1. Пользователь запускает Tensa на ПК. Desktop запускает дочерний
   `tensa-server.exe` с `TENSA_BIND=0.0.0.0:8765`.
2. Сервер читает `%USERPROFILE%\.tensa\config.toml` и `secrets.toml`.
3. PWA/виджет и Android обращаются к `/api/v1/*` по HTTP.
4. Локальные LLM/ASR-модели лежат в `Tensa\models` рядом с установленным
   приложением, а не в `~/.tensa/models`.
5. Для телефона сервер сообщает LAN-адрес через `GET /api/v1/server/info`;
   этот адрес показывается в настройках ПК над карточкой «Микрофон».

---

## 4. Сборка Windows

### 4.1. Предварительные требования

- Windows 10/11 x64;
- Rust stable / MSRV 1.95 и Cargo;
- Visual Studio Build Tools с C++ toolchain;
- Python 3.13 + Pillow (иконка);
- NSIS 3.11 (`makensis.exe`);
- `protoc`;
- libclang для `whisper-rs` / `transcribe-cpp`.

В текущем окружении использовались переменные:

```powershell
$env:LIBCLANG_PATH = "C:\tensa-tools\clang\native"
$env:PROTOC = "C:\tensa-tools\protoc\bin\protoc.exe"
$env:TENSA_MODELS_SRC = "C:\tensa-build\models"   # только e5-small
```

Пути локальные; в другом окружении достаточно указать свои значения.
Скрипты `.ps1` должны быть сохранены в **UTF-8 с BOM** — PowerShell 5.1
иначе читает русские строки как ANSI и ломает разбор.

### 4.2. Release-сборки

Из корня репозитория:

```powershell
cargo build --release -p tensa-server --features native
cargo build --release -p tensa-asr
cargo build --release -p tensa-desktop
```

`build_installer.ps1` дополнительно собирает `tensa-cli` и
`tensa-tools --example telegram_flow`, после чего раскладывает stage.

### 4.3. Установщик

```powershell
$env:LIBCLANG_PATH = "<...>"
$env:PROTOC = "<...>"
$env:TENSA_MODELS_SRC = "<каталог с multilingual-e5-small>"
powershell -ExecutionPolicy Bypass -File apps/desktop-lite/build_installer.ps1
```

Что делает скрипт:

1. собирает release-бинарники;
2. копирует `tensa-server.exe`, `tensa-desktop.exe`, `tensa-cli.exe`,
   `tensa-bot.exe`, `tensa-whisper.exe`, `tensa-asr.exe` в
   `C:\tensa-build\stage`;
3. копирует `pwa/static`, исключая `Tensa.apk`;
4. копирует `multilingual-e5-small` в `models`;
5. генерирует `tensa.ico` из `pwa/static/icons/icon-512.png`;
6. запускает `makensis` и создаёт `apps/desktop-lite/TensaSetup.exe`.

RUSTFLAGS внутри скрипта remap-ят путь профиля пользователя, чтобы приватные
пути не попадали в бинарники.

Результат: `apps/desktop-lite/TensaSetup.exe`.

---

## 5. Сборка Android

### 5.1. Требования

- JDK 17;
- Android SDK: `platforms;android-34`, `build-tools;34.0.0`;
- Python/Pillow не обязателен, если mipmap-иконки уже в `res`;
- PowerShell 5.1.

### 5.2. Команда

```powershell
powershell -ExecutionPolicy Bypass -File apps/android-lite/build_apk.ps1
```

Скрипт выполняет:

1. `aapt2 compile --dir res` — тема, mipmap-иконки;
2. `aapt2 link` с `--auto-add-overlay` — манифест, ресурсы, assets;
3. `javac` — Java-исходники (source/target 8);
4. `d8` — dex;
5. упаковку `classes.dex` в APK;
6. `zipalign`;
7. `apksigner` debug-ключом;
8. копию в `pwa/static/Tensa.apk` для раздачи с ПК.

Проверка:

```powershell
aapt2 dump badging apps/android-lite/Tensa.apk
aapt2 dump xmltree --file AndroidManifest.xml apps/android-lite/Tensa.apk
```

Ожидаемые значения: `versionCode=3`, `versionName=0.2.14`,
`launchable-activity=.MainActivity`, иконка `res/mipmap-*/ic_launcher.png`.

---

## 6. Первый запуск и отсутствие предустановленных ключей

### Windows

- `%USERPROFILE%\.tensa\config.toml` создаётся без выбранной модели;
- OpenRouter-ключ не задаётся;
- Gemma и ASR-модели отсутствуют в Setup;
- первый запуск без модели/ключа/ASR открывает «Настройки»;
- активные бейджи не отображаются (`activeBadges = []`);
- OpenRouter-поле показывает placeholder, а не сохранённый ключ.

### Android

- при пустом `Prefs.server` открывается `OnboardingActivity`;
- поля адреса и токена пустые;
- ключ OpenRouter не запрашивается на первом экране;
- «Настрою позже» оставляет приложение без подключения, без фиктивных
  активных статусов.

Токен ядра — `%USERPROFILE%\.tensa\pwa_token`; адрес сервера показывается
в настройках Windows-версии над «Микрофон».

---

## 7. ASR: почему стало быстрее

Раньше `tensa-asr.exe` запускался на каждую фразу и каждый раз загружал
модель (сотни мегабайт). Handy держит движок загруженным — это и было
главное отличие.

Сейчас:

- `tensa-asr --worker <model>` загружает модель **один раз**;
- сервер общается с worker по stdin/stdout JSON-строками;
- worker прогревается при старте сервера и после выбора модели;
- через worker идут Whisper и Parakeet/Canary/Nemotron;
- `tensa-whisper.exe` остаётся fallback для старых установок.

Измерения на тестовой фразе:

| Модель / режим | Было | Стало |
|---|---:|---:|
| Whisper small q5_1, HTTP | ~15.5 с | ~1.4 с |
| Parakeet TDT v3, после прогрева | ~1 с на запуск | ~0.1–0.2 с |

Первый запрос после холодного старта всё ещё включает загрузку модели;
именно поэтому добавлен preload.

---

## 8. Что изменилось в UI и логике

- логотип TensorLogix вместо буквы «T» в Windows/PWA/APK;
- маленький овал над микрофоном: «ЗАПИСЬ» / «РАСПОЗНАЮ…»;
- адрес сервера для телефона в настройках ПК над «Микрофон»;
- «этот отчёт» берётся из последней карточки файла;
- «первый/второй/третий/последний файл в списке» берётся из показанного
  списка, а не ищется заново;
- опечатки вида «Отопслаю … на почту» распознаются как email-команда;
- локальные файловые команды без модели отвечают списком/уточнением,
  а не HTTP 503;
- мобильный APK: onboarding, dashboard, чат с пузырями, системный голос,
  файлы и тёмная тема.

---

## 9. Проверки

| Проверка | Результат |
|---|---|
| GitHub Actions CI на `6836b51` | success |
| `cargo fmt --all --check` | чисто |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 предупреждений |
| `cargo test --workspace` (CI) | success |
| локальные серверные тесты | 33 passed |
| `cargo build --release -p tensa-server --features native` | успешно |
| `cargo build --release -p tensa-asr` | успешно |
| `cargo build --release -p tensa-desktop` | успешно |
| `build_installer.ps1` + NSIS | `TensaSetup.exe` создан |
| `build_apk.ps1` | APK собран и подписан |
| `aapt2 dump badging` | version 0.2.14, MainActivity, иконка |
| чистая установка Windows | настройки при первом запуске, активных бейджей нет |

Проверка артефактов:

```powershell
Get-FileHash apps/desktop-lite/TensaSetup.exe -Algorithm SHA256
Get-FileHash apps/android-lite/Tensa.apk -Algorithm SHA256
```

APK проверен на эмуляторе Genymotion (Android 15); детали — в §9.1.

### 9.1. Ручной тест Android (Genymotion + Trinity)

Устройство 1: **Xiaomi Redmi Note 9, Android 15**, Genymotion AVD
`192.168.21.101:5555`. APK установлен через ADB, версия `0.2.14`.

Проверено по шагам:

1. onboarding: поля пустые, введены адрес `http://192.168.1.101:8765` и
   токен из `pwa_token`;
2. после «Проверить и подключиться» — dashboard, статус **«ПК на связи»**;
3. чат: сообщение `find` обработано локальным поиском, ответ
   «Ничего не нашёл…» вместо HTTP 503;
4. файлы: экран поиска открывается, запрос уходит в индекс ПК;
5. микрофон: разрешение выдано, показан овальный маяк **«СЛУШАЮ…»**,
   после остановки — **«РАСПОЗНАЮ…»**;
6. серверный ASR: у Genymotion нет системного распознавателя и реального
   аудиовхода, поэтому запрос дошёл до `/api/v1/voice/transcribe` и вернул
   «речь не распознана»; это подтверждает fallback-путь, но не качество
   распознавания с живым микрофоном.

BlueStacks Pie64 запускался и был виден ADB, но при передаче APK
(228 МБ) соединение падало с `protocol fault`, поэтому полный сценарий
на нём не выполнялся. Genymotion прошёл установку и UI-сценарий.

Устройство 2: **Trinity Android-x86, Android 9**, эмулятор скачан с
GitHub (`TrinityEmulator/TrinityEmulator`, asset `Trinity.zip` ~1 ГБ).
Genymotion AVD Redmi 9 удалён; Trinity запущен через `Trinity.exe`,
APK установлен через `127.0.0.1:5555`.

На Trinity проверено: приложение отрисовывается, onboarding принимает
адрес/токен, после подключения dashboard показывает **«ПК на связи»** и
адрес `http://192.168.1.101:8765`. Trinity работает в landscape
1920×1080, поэтому это дополнительная проверка широкого экрана.

Скриншоты: [`mobile-onboarding.png`](docs/screenshots/mobile-onboarding.png),
[`mobile-dashboard.png`](docs/screenshots/mobile-dashboard.png),
[`mobile-chat.png`](docs/screenshots/mobile-chat.png),
[`mobile-voice-beacon.png`](docs/screenshots/mobile-voice-beacon.png),
[`trinity-dashboard.png`](docs/screenshots/trinity-dashboard.png).

---

## 10. Безопасность и данные

- секреты не коммитятся в git;
- токен PWA хранится в `%USERPROFILE%\.tensa\pwa_token`;
- SMTP/Telegram-секреты — в `%USERPROFILE%\.tensa\secrets.toml`;
- секреты не логируются;
- временные WAV удаляются после распознавания;
- Setup/APK не содержат OpenRouter-ключа;
- в отчёте и релизных notes нет токенов и паролей.

Ограничение: `Secrets::openrouter_key()` по-прежнему читает
`OPENROUTER_API_KEY` из окружения с приоритетом над файлом. На чистой машине
переменной нет; если она заранее задана в Windows, сервер может увидеть её
раньше пользовательского ввода. Это следует учитывать при проверке
«ничего не активно».

---

## 11. История релизов

| Версия | Что вошло |
|---|---|
| v0.2.8 | модели в каталоге установки, лёгкий Setup без Whisper-моделей |
| v0.2.9 | логотип TensorLogix для Windows/PWA/APK |
| v0.2.10 | контекст «этот отчёт» из последней карточки |
| v0.2.11 | постоянный ASR-worker, маяк записи, адрес сервера в настройках |
| v0.2.12 | мобильный APK: onboarding, dashboard, чат, голос, файлы |
| v0.2.13 | «первый файл в списке», опечатки email-команд, локальные файловые ответы |
| v0.2.14 | Android: fallback ASR через ПК, тест на Genymotion и Trinity |

---

## 12. Известные ограничения

1. APK подписан debug-ключом; это демо-сборка, не production-подпись.
2. На телефоне нет локальной LLM/ASR; голос — системный `SpeechRecognizer`
   Android, поэтому качество/офлайн-поведение зависит от телефона.
3. APK и Setup содержат e5-small; LLM Gemma и ASR-модели скачиваются.
4. SMTP/Telegram с новыми реальными кредами не перепроверялись в этом
   окружении.
5. Старый `~/.tensa/models` может использоваться как fallback, если
   `asr-active.txt` ссылается на файл там; основной путь — каталог установки.
6. `OPENROUTER_API_KEY` в окружении имеет приоритет над файлом (см. §10).
7. Мобильный APK проверен на Genymotion Android 15 и Trinity Android 9.
   На BlueStacks Pie64 установка 228-МБ APK не завершилась (`protocol fault`).
   Реальный микрофон и системный SpeechRecognizer на эмуляторах недоступны,
   поэтому качество голосового распознавания с живым микрофоном ещё нужно
   проверить на телефоне.

---

## 13. Воспроизведение релиза

```powershell
git clone https://github.com/kostyuk230378-cmd/TENSA.git
cd TENSA
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Windows
cargo build --release -p tensa-server --features native
cargo build --release -p tensa-asr
cargo build --release -p tensa-desktop
$env:TENSA_MODELS_SRC = "<путь к multilingual-e5-small>"
powershell -ExecutionPolicy Bypass -File apps/desktop-lite/build_installer.ps1

# Android
powershell -ExecutionPolicy Bypass -File apps/android-lite/build_apk.ps1

# Публикация
gh release create vX.Y.Z apps/desktop-lite/TensaSetup.exe apps/android-lite/Tensa.apk `
  --title "vX.Y.Z" --notes-file RELEASE_NOTES.md
```

---

## Ссылки

- Репозиторий: <https://github.com/kostyuk230378-cmd/TENSA>
- Релизы: <https://github.com/kostyuk230378-cmd/TENSA/releases>
- CI: <https://github.com/kostyuk230378-cmd/TENSA/actions>
- README: <https://github.com/kostyuk230378-cmd/TENSA/blob/main/README.md>

_Отчёт подготовлен по состоянию `main` на 21.10.2026._
