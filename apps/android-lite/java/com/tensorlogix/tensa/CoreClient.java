package com.tensorlogix.tensa;

import java.io.IOException;
import java.io.InputStream;
import java.io.UnsupportedEncodingException;
import java.net.URLEncoder;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Клиент локального ядра Tensa (HTTP /api/v1, Bearer-авторизация).
 *
 * Все методы синхронные — вызывать через {@link Background}.
 * Ошибки — исключениями с понятным текстом сервера (`error` из JSON).
 */
public final class CoreClient {

    private CoreClient() {
    }

    /** Проверить, что адрес ядра задан. */
    private static void requireServer(String server) {
        if (server == null || server.trim().isEmpty()) {
            throw new IllegalStateException("Не задан адрес ядра — откройте Настройки");
        }
    }

    /** Проверить, что Bearer задан. */
    private static void requireBearer(String bearer) {
        if (bearer == null || bearer.trim().isEmpty()) {
            throw new IllegalStateException("Не задан токен ядра — откройте Настройки");
        }
    }

    /** Сообщение об ошибке из тела ответа (`{"error": "..."}`). */
    private static String errorMessage(Http.Response response) {
        String raw = response.body == null ? "" : response.body.trim();
        try {
            JSONObject json = new JSONObject(raw);
            String message = json.optString("error", "");
            if (!message.isEmpty()) {
                return message;
            }
        } catch (Exception ignored) {
            // не JSON — показываем как есть
        }
        return raw.length() > 200 ? raw.substring(0, 200) + "…" : raw;
    }

    /** GET с проверкой статуса. */
    private static String get(String server, String bearer, String path) throws Exception {
        requireServer(server);
        requireBearer(bearer);
        Http.Response response = Http.get(server + path, bearer.trim());
        if (!response.ok()) {
            throw new IOException(errorMessage(response));
        }
        return response.body;
    }

    /** POST с проверкой статуса. */
    private static String post(String server, String bearer, String path, String body)
            throws Exception {
        requireServer(server);
        requireBearer(bearer);
        Http.Response response = Http.postJson(server + path, bearer.trim(), body);
        if (!response.ok()) {
            throw new IOException(errorMessage(response));
        }
        return response.body;
    }

    /** DELETE с проверкой статуса. */
    private static void delete(String server, String bearer, String path) throws Exception {
        requireServer(server);
        requireBearer(bearer);
        Http.Response response = Http.delete(server + path, bearer.trim());
        if (!response.ok()) {
            throw new IOException(errorMessage(response));
        }
    }

    /** Состояние ядра (JSON HealthStatus). */
    public static JSONObject health(String server, String bearer) throws Exception {
        return new JSONObject(get(server, bearer, "/api/v1/health"));
    }

    /** Очередь одобрения (JSON-массив действий). */
    public static JSONArray approvals(String server, String bearer) throws Exception {
        return new JSONArray(get(server, bearer, "/api/v1/approvals"));
    }

    /** Подтвердить действие. */
    public static void approve(String server, String bearer, String id) throws Exception {
        post(server, bearer, "/api/v1/approvals/" + id + "/approve", "{}");
    }

    /** Отклонить действие. */
    public static void reject(String server, String bearer, String id) throws Exception {
        post(server, bearer, "/api/v1/approvals/" + id + "/reject", "{}");
    }

    /** Запрос к модели через ядро (облако/локальная GGUF). */
    public static JSONObject query(String server, String bearer, String text) throws Exception {
        JSONObject body = new JSONObject();
        body.put("text", text);
        return new JSONObject(post(server, bearer, "/api/v1/query", body.toString()));
    }

    /** Заметки (свежие первыми). */
    public static JSONArray notes(String server, String bearer) throws Exception {
        return new JSONArray(get(server, bearer, "/api/v1/notes?limit=20&offset=0"));
    }

    /** Создать заметку. */
    public static void createNote(String server, String bearer, String title, String body)
            throws Exception {
        JSONObject note = new JSONObject();
        note.put("title", title);
        note.put("body", body);
        note.put("tags", new JSONArray());
        post(server, bearer, "/api/v1/notes", note.toString());
    }

    /** Удалить заметку (мягкое удаление на стороне ядра). */
    public static void deleteNote(String server, String bearer, String id) throws Exception {
        delete(server, bearer, "/api/v1/notes/" + id);
    }

    /** Поиск по заметкам (FTS5). */
    public static JSONArray searchNotes(String server, String bearer, String query, int limit)
            throws Exception {
        String encoded = encode(query);
        return new JSONArray(get(server, bearer,
                "/api/v1/notes/search?q=" + encoded + "&limit=" + limit));
    }

    /** Почтовые аккаунты из конфига ядра. */
    public static JSONArray emailAccounts(String server, String bearer) throws Exception {
        return new JSONArray(get(server, bearer, "/api/v1/email/accounts"));
    }

    /** События календаря (все, по возрастанию начала). */
    public static JSONArray calendarEvents(String server, String bearer) throws Exception {
        return new JSONArray(get(server, bearer, "/api/v1/calendar/events"));
    }

    /** Создать событие; конфликт приходит IOException с текстом сервера (409). */
    public static JSONObject createEvent(String server, String bearer, String title,
                                         long startsAt, long endsAt, String location)
            throws Exception {
        JSONObject body = new JSONObject();
        body.put("title", title);
        body.put("starts_at", startsAt);
        body.put("ends_at", endsAt);
        if (location != null && !location.trim().isEmpty()) {
            body.put("location", location.trim());
        }
        return new JSONObject(post(server, bearer, "/api/v1/calendar/events", body.toString()));
    }

    /** Статус модели эмбеддингов у ядра. */
    public static JSONObject embedderStatus(String server, String bearer) throws Exception {
        return new JSONObject(get(server, bearer, "/api/v1/models/embedder"));
    }

    /** Передать один файл модели в ядро (поток из assets). */
    public static void uploadEmbedderFile(String server, String bearer, String name,
                                          InputStream stream) throws Exception {
        requireServer(server);
        requireBearer(bearer);
        Http.Response response = Http.postStream(
                server + "/api/v1/models/embedder/" + name,
                bearer.trim(), stream, "application/octet-stream");
        if (!response.ok()) {
            throw new IOException(errorMessage(response));
        }
    }

    /** URL-кодирование параметра. */
    private static String encode(String value) {
        try {
            return URLEncoder.encode(value, "UTF-8");
        } catch (UnsupportedEncodingException error) {
            return value;
        }
    }
}
