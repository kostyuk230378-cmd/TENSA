package com.tensorlogix.tensa;

import java.io.IOException;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Клиент локального ядра Tensa (HTTP /api/v1, Bearer-токен).
 *
 * Все методы синхронные — вызывать через {@link Background}.
 * Ошибки — исключениями с понятным текстом.
 */
public final class CoreClient {

    private CoreClient() {
    }

    /** Проверить, что токен задан. */
    private static void requireToken(String token) {
        if (token == null || token.trim().isEmpty()) {
            throw new IllegalStateException("Не задан токен ядра — откройте Настройки");
        }
    }

    /** GET с проверкой статуса. */
    private static String get(String server, String token, String path) throws Exception {
        requireToken(token);
        Http.Response response = Http.get(server + path, token.trim());
        if (!response.ok()) {
            throw new IOException("ядро HTTP " + response.status + ": " + shorten(response.body));
        }
        return response.body;
    }

    /** POST с проверкой статуса. */
    private static String post(String server, String token, String path, String body)
            throws Exception {
        requireToken(token);
        Http.Response response = Http.postJson(server + path, token.trim(), body);
        if (!response.ok()) {
            throw new IOException("ядро HTTP " + response.status + ": " + shorten(response.body));
        }
        return response.body;
    }

    /** DELETE с проверкой статуса. */
    private static void delete(String server, String token, String path) throws Exception {
        requireToken(token);
        Http.Response response = Http.delete(server + path, token.trim());
        if (!response.ok()) {
            throw new IOException("ядро HTTP " + response.status + ": " + shorten(response.body));
        }
    }

    /** Состояние ядра (JSON HealthStatus). */
    public static JSONObject health(String server, String token) throws Exception {
        return new JSONObject(get(server, token, "/api/v1/health"));
    }

    /** Очередь одобрения (JSON-массив действий). */
    public static JSONArray approvals(String server, String token) throws Exception {
        return new JSONArray(get(server, token, "/api/v1/approvals"));
    }

    /** Подтвердить действие. */
    public static void approve(String server, String token, String id) throws Exception {
        post(server, token, "/api/v1/approvals/" + id + "/approve", "{}");
    }

    /** Отклонить действие. */
    public static void reject(String server, String token, String id) throws Exception {
        post(server, token, "/api/v1/approvals/" + id + "/reject", "{}");
    }

    /** Заметки (свежие первыми). */
    public static JSONArray notes(String server, String token) throws Exception {
        return new JSONArray(get(server, token, "/api/v1/notes?limit=20&offset=0"));
    }

    /** Создать заметку. */
    public static void createNote(String server, String token, String title, String body)
            throws Exception {
        JSONObject note = new JSONObject();
        note.put("title", title);
        note.put("body", body);
        note.put("tags", new JSONArray());
        post(server, token, "/api/v1/notes", note.toString());
    }

    /** Удалить заметку (мягкое удаление на стороне ядра). */
    public static void deleteNote(String server, String token, String id) throws Exception {
        delete(server, token, "/api/v1/notes/" + id);
    }

    /** Укоротить тело ошибки для показа. */
    private static String shorten(String text) {
        String value = text == null ? "" : text.trim();
        return value.length() > 200 ? value.substring(0, 200) + "…" : value;
    }
}
