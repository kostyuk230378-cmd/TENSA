# tensa-server

HTTP-бэкенд PWA Tensa (Контракт 12): axum-роутер поверх крейтов воркспейса.

## Состав MVP

| Маршрут (базовый путь `/api/v1`) | Назначение |
| --- | --- |
| `GET /health` | Сводный статус ядра (реальные пробы хранилищ) |
| `GET /version` | Версия крейта (JSON-строка) |
| `GET /approvals`, `GET /approvals/{id}` | Очередь одобрения |
| `POST /approvals/{id}/approve`, `POST /approvals/{id}/reject` | Решение по действию |
| `GET/POST /notes`, `GET/PUT/DELETE /notes/{id}`, `GET /notes/search?q=` | Заметки |
| `GET /events` | SSE-поток (`welcome`, `status:health`, `approval:updated`) |
| `GET /`, `GET /app`, `GET /static/...` | Статика PWA (публична, без токена) |
| Остальные маршруты контракта | `501 Not Implemented` (заглушки в `routes::stubs`) |

## Доступ

Все API-маршруты требуют `Authorization: Bearer <токен>`.
Токен читается из файла `<корень данных>/pwa_token`; если файла нет —
генерируется (32 байта энтропии ОС, hex) и записывается. Значение токена
не логируется: в лог уходит только путь файла.

## Точки входа

- `tensa_server::router(state)` — собрать приложение (для тестов и встраивания);
- `tensa_server::serve(addr, state)` — запустить сервер (ТЗ: `127.0.0.1:8765`);
- `tensa_server::load_or_create_token(root)` — доступ PWA.

## Проверки

```powershell
cargo test -p tensa-server
cargo clippy -p tensa-server --all-targets -- -D warnings
```

## Ограничения MVP

- Статика: каталог `pwa/static` из ТЗ; если его нет — HTML-заглушка.
- Список одобрений отдаёт до лимита хранилища; пагинация — вне MVP.
- Реализованы не все маршруты контракта: остальные отвечают 501 осознанно,
  чтобы клиент, сгенерированный из `openapi.yaml`, отличал «ещё нет» от 404.
