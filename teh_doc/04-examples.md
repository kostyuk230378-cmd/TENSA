# Tensa — Примеры кода и референсы

**Версия:** 0.1.0
**Дата:** 2026-10-06
**Статус:** Собраны примеры, использованные при проектировании

---

## Содержание

1. [Telegram-бот (aiogram + SQLite)](#telegram-bot)
2. [PDF-детект (is_scanned_pdf)](#pdf-detect)
3. [Debounce (500ms)](#debounce)
4. [LLM-тесты (tool use, reasoning, сложный сценарий)](#llm-tests)
5. [Плавающий виджет (концепция)](#floating-widget)
6. [Референсы (что смотрели)](#references)

---

## Telegram-бот (aiogram + SQLite)

**Источник:** пример, который ты отправлял для общего развития.

**Назначение:** показать архитектуру «Telegram → Handler → DB Manager» с контролем доступа.

**Код:**

```python
# config.py
import os
from dotenv import load_dotenv

load_dotenv()

BOT_TOKEN = os.getenv("TELEGRAM_BOT_TOKEN")
DB_PATH = "user_database.sqlite"
```

```python
# db_manager.py
import sqlite3
from config import DB_PATH

def get_db_connection():
    try:
        conn = sqlite3.connect(DB_PATH)
        return conn
    except sqlite3.Error as e:
        print(f"❌ Ошибка подключения к базе данных: {e}")
        return None

def setup_database():
    conn = get_db_connection()
    if conn:
        cursor = conn.cursor()
        cursor.execute(
            "CREATE TABLE IF NOT EXISTS users "
            "(id INTEGER PRIMARY KEY, name TEXT, telegram_id INTEGER)"
        )
        conn.commit()
        print("✅ База данных инициализирована.")

def insert_user(name: str, telegram_id: int):
    conn = get_db_connection()
    if conn:
        try:
            cursor = conn.cursor()
            cursor.execute(
                "INSERT INTO users (name, telegram_id) VALUES (?, ?)",
                (name, telegram_id)
            )
            conn.commit()
            return True
        except sqlite3.Error as e:
            print(f"❌ Ошибка SQL при вставке данных: {e}")
            return False
    return False

def fetch_user(telegram_id: int):
    conn = get_db_connection()
    if conn:
        cursor = conn.cursor()
        cursor.execute(
            "SELECT id, name FROM users WHERE telegram_id = ?",
            (telegram_id,)
        )
        result = cursor.fetchone()
        if result:
            return {"id": result[0], "name": result[1]}
        return None
    return None
```

```python
# handlers/register_handler.py
from aiogram import Router, F
from aiogram.types import Message
from db_manager import setup_database, insert_user, fetch_user

router = Router()

@router.message(F.text == "/register")
async def register_user(message: Message):
    try:
        name = "User_from_Telegram"
        telegram_id = message.from_user.id
    except Exception as e:
        await message.answer(
            "❌ Не удалось извлечь данные для регистрации."
        )
        return

    setup_database()

    if insert_user(name, telegram_id):
        await message.answer("✅ **Успех!** Профиль создан.")
    else:
        await message.answer("❌ Ошибка при сохранении данных.")
```

```python
# main.py
import asyncio
from aiogram import Application
from config import BOT_TOKEN
from handlers.register_handler import router as register_router
import logging

logging.basicConfig(level=logging.INFO)

async def main():
    app = Application.builder().token(BOT_TOKEN).build()
    app.include_router(register_router)
    logging.info("Бот запущен и слушает команды...")
    await app.run_polling()

if __name__ == "__main__":
    asyncio.run(main())
```

**Что взято в Tensa:**
- Чистое разделение модулей (config, db_manager, handlers).
- Проверка `telegram_id` (в Tensa — `owner_chat_id`).
- SQLite для хранения.
- Логирование.

**Отличия Tensa:**
- Rust, а не Python.
- Не регистрация, а фиксированный владелец.
- Не только SQLite, а ApprovalStore + Nodes + Notes + Lance.
- Не только Telegram, а почта, WhatsApp, календарь, приложения.

---

## PDF-детект (is_scanned_pdf)

**Источник:** код, который ты отправлял для проверки определения сканированных PDF.

**Назначение:** эвристика для определения, является ли страница PDF сканированной (нужен OCR) или текстовой.

**Код (начальная версия):**

```rust
fn is_scanned_pdf(pdf_path: &str) -> bool {
    let pdf = Pdf::open_path(pdf_path).unwrap();
    let mut total_chars = 0;
    let mut total_images = 0;

    for page in pdf.pages() {
        total_chars += page.chars().len();
        total_images += page.images().len();
    }

    // Эвристика: если на весь документ почти нет букв,
    // но есть тяжелые картинки — это 100% скан.
    total_chars < 50 && total_images > 0
}
```

**Код (постраничная версия):**

```rust
fn is_scanned_pdf(page: &PdfPage) -> bool {
    let char_count = page.chars().len();
    let image_count = page.images().len();

    // 1. Если текста вообще нет, а картинки есть — это 100% скан
    if char_count == 0 && image_count > 0 {
        return true;
    }

    // 2. Если текста подозрительно мало (меньше 100 символов)
    // AND при этом на странице есть хотя бы одна тяжелая картинка
    if char_count < 100 && image_count > 0 {
        return true;
    }

    // 3. Защита от "мусорного" скрытого OCR:
    // Проверяем среднюю длину слов. Если из 120 символов нельзя
    // собрать нормальные слова — это скан.
    let text: String = page.chars().iter().map(|c| c.text()).collect();
    let words: Vec<&str> = text.split_whitespace().collect();
    if !words.is_empty() {
        let avg_word_len: f64 = words.iter().map(|w| w.len()).sum::<usize>() as f64
            / words.len() as f64;
        if avg_word_len < 2.0 {
            return true;
        }
    }

    false
}
```

**Что взято в Tensa:**
- Постраничное определение (ADR-020).
- Правило 1: нет текста + есть картинки = скан.
- Правило 2: мало текста (<100) + есть картинки = скан.
- Правило 3: защита от мусорного OCR (средняя длина слов < 2.0).

---

## Debounce (500ms)

**Источник:** код, который ты отправлял для проверки соединения (Вариант D).

**Назначение:** не спамить сервер на каждый введённый символ, проверять соединение только когда пользователь закончил ввод.

**Код:**

```rust
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

async fn watch_url_input(mut rx: mpsc::Receiver<String>) {
    let mut last_input = String::new();

    while let Some(url) = rx.recv().await {
        last_input = url.clone();
        // Даем пользователю 500мс, чтобы дописать адрес
        sleep(Duration::from_millis(500)).await;

        // Если за 500мс новых символов не пришло — проверяем
        if url == last_input {
            spawn_connection_check(url).await;
        }
    }
}

async fn spawn_connection_check(url: String) {
    // Отправляем легкий фоновый HTTP-запрос
    // Меняем статус в UI: Connecting -> Success/Error
}
```

**Вариант D (полный):**
1. **Интеллектуальный ввод (Debounce 500ms):** Tensa ждёт 500ms после того, как пользователь перестал набирать.
2. **Асинхронный UI-индикатор:** spinner рядом с полем ввода, интерфейс не блокируется.
3. **Визуализация статуса:**
   - Зелёный чекбокс: соединение установлено, API-ключ валиден, модель доступна. Кнопка «Сохранить» активна.
   - Красный текст: «Не удалось подключиться» или «Неверный API-ключ». Кнопка «Сохранить» заблокирована.
4. **Валидация при сохранении (Вариант B):** финальная быстрая проверка при нажатии «Сохранить».

**Что взято в Tensa:**
- Debounce 500ms (ADR-015, контракт 6).
- Асинхронный UI-индикатор.
- Визуализация статуса.
- Финальная валидация при сохранении.

---

## LLM-тесты (tool use, reasoning, сложный сценарий)

**Источник:** тесты, которые ты проводил на TheStageAI/gemma-4-E2B-it (Q3_K_S и Q4_K_S).

**Назначение:** проверить, справляется ли модель с задачами секретаря на русском.

### Тест 1: Русский + JSON

**Промпт:**
```
Ты — секретарь. Отвечай кратко, по делу, на русском языке.

Задача: Пользователь просит: «Найди смету за апрель и перешли в Telegram».

Ответь в формате JSON:
{
  "action": "название действия",
  "params": { "ключ": "значение" },
  "reasoning": "краткое объяснение"
}

Не добавляй ничего лишнего. Только JSON.
```

**Результат XS (Q3_K_S):**
```json
{
  "action": "Поиск и Пересылка Данных",
  "params": { "ключ": "смета_апрель", "destination": "Telegram" },
  "reasoning": "Выполняется запрос на поиск сметы за апрель..."
}
```

**Результат S (Q4_K_S):**
```json
{
  "action": "Поиск и Пересылка Документа",
  "params": { "ключ": "смета_апрель", "destination": "Telegram" },
  "reasoning": "Выполняется поиск сметы за апрель..."
}
```

**Вывод:** обе модели понимают задачу, соблюдают JSON, но `action` описательный (не стандартизированный). Решается улучшенным промптом / GBNF.

### Тест 2: Tool use

**Промпт:**
```
Ты — секретарь с доступом к инструментам. Отвечай ТОЛЬКО JSON.

Доступные инструменты:
- search_files: { "query": string }
- send_email: { "to": string, "subject": string, "body": string }
- send_telegram: { "message": string }
- create_calendar_event: { "title": string, "start": string, "end": string }

Задача: «Отправь Ивану письмо с темой "Смета" и текстом "Во вложении"».

Ответ:
```

**Результат XS:**
```json
{
  "tool_call": {
    "name": "send_email",
    "params": { "to": "Иван", "subject": "Смета", "body": "Во вложении" }
  }
}
```

**Результат S:**
```json
{
  "tool_name": "send_email",
  "params": { "to": "Иван", "subject": "Смета", "body": "Во вложении" }
}
```

**Вывод:** обе модели справились. Разница в названии поля (`tool_call.name` vs `tool_name`). Решается GBNF.

### Тест 3: Reasoning

**Промпт:**
```
Ты — секретарь. Отвечай на русском языке.

Задача: «Назначь встречу с Петром на следующую среду в 15:00, но проверь, нет ли конфликтов с моим календарём».

Объясни пошагово:
Шаг 1: ...
Шаг 2: ...
Шаг 3: ...
```

**Результат XS:**
```
Шаг 1: Определю точную дату и время для встречи ("следующая среда, 15:00").
Шаг 2: Проверю доступность этого времени в моём календарём (имитируя проверку конфликтов).
Шаг 3: Если нет конфликтов, я использую инструмент create_calendar_event для назначения встречи с Петром.
```

**Результат S:**
```
Шаг 1: Определить точную дату и время встречи (следующая среда, 15:00).
Шаг 2: Проверить доступность времени в календаре на предмет конфликтов с существующими событиями.
Шаг 3: Если конфликтов нет, использовать инструмент create_calendar_event для назначения встречи с Петром.
```

**Вывод:** обе модели справились. XS допустил грамматическую ошибку («в моём календарём»), S — чище.

### Тест 4: Сложный сценарий

**Промпт:**
```
Ты — секретарь Tensa. Отвечай ТОЛЬКО JSON.

Контекст:
- Сегодня 2026-10-04, воскресенье.

Доступные инструменты:
- search_files: { "query": string }
- send_email: { "to": string, "subject": string, "body": string }
- send_telegram: { "message": string }
- create_calendar_event: { "title": string, "start": string, "end": string }
- check_calendar: { "start": string, "end": string }

Задача: «Найди смету за апрель, отправь её Ивану на почту и назначь встречу с ним на завтра в 10:00».

Ответ в формате:
{
  "steps": [ { "tool": "...", "params": { ... } } ],
  "reasoning": "..."
}
```

**Результат XS:**
```json
{
  "steps": [
    { "tool": "search_files", "params": { "query": "смета за апрель" } },
    { "tool": "send_email", "params": { "to": "Ivan", "subject": "Смета", "body": "Во вложении" } },
    { "tool": "create_calendar_event", "params": { "title": "Встреча с Иваном", "start": "2026-10-05 10:00", "end": "2026-10-05 12:00" } }
  ],
  "reasoning": "Для выполнения задачи необходимо выполнить три последовательные операции..."
}
```

**Результат S:**
```json
{
  "steps": [
    { "tool": "search_files", "params": { "query": "смета за апрель" } },
    { "tool": "send_email", "params": { "to": "Иван", "subject": "Смета Апреля", "body": "Во вложении сметы за апрель." } },
    { "tool": "check_calendar", "params": { "start": "2026-10-05 10:00", "end": "2026-10-05 11:00" } },
    { "tool": "create_calendar_event", "params": { "title": "Встреча с Иваном", "start": "2026-10-05 10:00", "end": "2026-10-05 11:00" } }
  ],
  "reasoning": "Сначала ищем нужный файл. Затем отправляем его Ивану по почте..."
}
```

**Вывод:**
- **S справился лучше:** включил `check_calendar`, использовал `Иван` (кириллица), длительность 1 час.
- **XS пропустил `check_calendar`**, использовал `Ivan` (латиница), длительность 2 часа.

**Решение:** для сложных сценариев — S (Q4_K_S, 2.40 ГБ) или GBNF-грамматика.

### Итоговый вывод по LLM

- **XS (Q3_K_S, 1.73 ГБ)** — достаточно для простых задач (tool use, reasoning).
- **S (Q4_K_S, 2.40 ГБ)** — лучше для сложных сценариев.
- **Решение:** динамический выбор tier по RAM (XS для слабых, S для мощных).
- **Проблемы:** нестандартные `action`, формат даты, `Ivan` vs `Иван` — решаются GBNF и улучшенным промптом.

---

## Плавающий виджет (концепция)

**Источник:** концепция, которую ты описал (изображение + логика).

**Назначение:** единый виджет для Android и ПК, две зоны, адаптивный ввод.

### Визуализация

Компактный плавающий полупрозрачный значок-виджет в цветах брендбука:
- **Левая круглая зона (кнопка «Т»)**
- **Правая вытянутая зона (три точки «...»)**

### Логика левой зоны (кнопка «Т»)

- **Тап/клик** — мгновенная активация ввода.
- **Если ввод = Голос:** значок превращается в пульсирующую звуковую волну, слушает. **ОДНОВРЕМЕННО** разворачивается небольшое окно с распознанным текстом и командами, с возможностью редактирования с клавиатуры.
- **Если ввод = Текст:** виджет плавно раздвигается вбок, открывая компактное диалоговое окно с полем ввода.

### Логика правой зоны (три точки «...»)

- **Тап/клик** — быстрое контекстное микро-меню.
- **Элементы меню:** [Переключить ввод: Голос/Текст], [Открыть архив найденных файлов], [Перейти в полные Настройки].

### Состояния виджета

1. Свёрнутый — компактный значок (Т + ...).
2. Голосовой — пульсирующая волна + окно с текстом.
3. Текстовый — раздвинутое окно с полем ввода.
4. Меню — контекстное микро-меню.
5. Свёрнутый в край — половина значка у края экрана.

### Настройки виджета

- Позиция (по умолчанию — правый нижний угол).
- Прозрачность (по умолчанию — 70%).
- Размер (маленький, средний, большой).
- Режим по умолчанию: голос / текст.
- Авто-сворачивание в край.
- Android: overlay + App Widget (оба, настройка в UI).
- ПК: отдельное окно (always on top, transparent).

### Реализация

**Android (Flutter):**
- `SYSTEM_ALERT_WINDOW` — overlay.
- `AppWidgetProvider` — App Widget.
- `Foreground Service` — постоянное присутствие.
- `WindowManager` — управление overlay.
- Flutter — UI (анимации встроенные).

**ПК (Tauri + Svelte):**
- Отдельное окно `widget` (always on top, transparent, skip taskbar).
- `tauri-plugin-global-shortcut` — хоткей.
- Rust-бэкенд — управление окном.
- Svelte — UI (CSS-анимации).

**Голос:**
- Android: `sherpa-onnx` (локально) или gRPC к ПК.
- ПК: `whisper.cpp`.
- Не Handy.

---

## Референсы (что смотрели)

При проектировании Tensa были рассмотрены следующие проекты и референсы:

### OCR

| Проект | Лицензия | Что даёт | Решение |
|---|---|---|---|
| RustO! (byrizki/rusto-rs) | MIT | Чистый Rust, MNN, PP-OCRv6, Android/iOS SDK | **Выбран** |
| oar-ocr (GreatV/oar-ocr) | Apache-2.0 | Rust + ONNX Runtime, PP-OCRv6 | Альтернатива |
| franken_ocr (Dicklesworthstone) | MIT + rider | Чистый Rust, PDF | Референс (rider — риск) |
| PaddleOCR + Paddle Lite | Apache-2.0 | C++/Java, не Rust | Fallback для Android |

### PDF

| Проект | Лицензия | Что даёт | Решение |
|---|---|---|---|
| developer0hye/pdfplumber-rs | Apache-2.0 | Лёгкий, чистый Rust | **Выбран** |
| clark-labs-inc/pdfsiak-rs | Apache-2.0 | 10–50x быстрее Python | Альтернатива |
| firecrawl/pdf-inspector | Apache-2.0 | Markdown, определение сканов | Альтернатива |

### DOCX, XLSX, PPTX

| Формат | Проект | Лицензия | Решение |
|---|---|---|---|
| DOCX | dgriffen/docx-rs | MIT/Apache | **Выбран** |
| XLSX | calamine | MIT | **Выбран** |
| PPTX | Свой на zip + quick-xml | MIT/Apache | **Выбран** |

### LLM

| Проект | Что даёт | Решение |
|---|---|---|
| llama.cpp | C++ движок, GGUF | **Выбран** (FFI) |
| llama-cpp-rs | Rust-биндинги | Альтернатива |
| candle | Rust-фреймворк HF | Альтернатива |
| ort (ONNX Runtime) | ONNX-модели | Не подходит (нет GGUF) |

### Модели

| Модель | Размер | Назначение | Решение |
|---|---|---|---|
| TheStageAI/gemma-4-E2B-it-GGUF (XS, Q3_K_S) | 1.73 ГБ | Локальная LLM (слабые устройства) | **Выбран** |
| TheStageAI/gemma-4-E2B-it-GGUF (S, Q4_K_S) | 2.40 ГБ | Локальная LLM (средние) | **Выбран** |
| TheStageAI/gemma-4-E2B-it-GGUF (M, Q4_K_M) | 2.68 ГБ | Локальная LLM (мощные) | **Выбран** |
| google/gemma-4-26b-a4b-it:free | — | Облачная LLM (бесплатная) | **Выбран** |
| google/gemma-4-26b-a4b-it | — | Облачная LLM (платная) | **Выбран** |
| intfloat/multilingual-e5-small | 130 МБ | Эмбеддинги | **Выбран** |
| snakers4/silero-vad | 2 МБ | VAD | **Выбран** |
| whisper.cpp (ggml-base.bin) | 142 МБ | ASR (десктоп) | **Выбран** |
| sherpa-onnx-whisper-tiny | 39 МБ | ASR (Android) | **Выбран** |
| Cyrillic OCR (PaddleOCR) | — | OCR (кириллица) | **Выбран** |

### Библиотеки

| Компонент | Проект | Лицензия | Решение |
|---|---|---|---|
| Векторный поиск | Lance 13.0 | Apache-2.0 | **Выбран** |
| Эмбеддинги | fastembed-rs 7.1 | Apache-2.0 | **Выбран** |
| FFI | uniffi | MPL-2.0 | **Выбран** |
| gRPC | tonic | MIT | **Выбран** |
| Telegram | teloxide | MIT | **Выбран** |
| IMAP | himalaya | MIT | **Выбран** |
| SMTP | lettre | MIT | **Выбран** |
| CalDAV | caldav | MIT/Apache | **Выбран** |
| Syncthing | syncthing-android | MPL-2.0 | **Выбран** |
| PDF | pdfplumber-rs | Apache-2.0 | **Выбран** |
| DOCX | docx-rs | MIT/Apache | **Выбран** |
| XLSX | calamine | MIT | **Выбран** |
| OCR | RustO! | MIT | **Выбран** |
