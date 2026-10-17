package com.tensorlogix.tensa;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

/**
 * Мини-HTTP поверх HttpURLConnection (без сторонних библиотек).
 *
 * Используется и для ядра Tensa (Bearer-токен), и для OpenRouter (ключ).
 * Таймауты ограничены, ошибки — исключениями; секреты не логируются.
 */
public final class Http {

    /** Таймаут соединения, мс. */
    private static final int CONNECT_TIMEOUT_MS = 15000;
    /** Таймаут чтения, мс (LLM отвечает не мгновенно). */
    private static final int READ_TIMEOUT_MS = 120000;

    private Http() {
    }

    /** Ответ: HTTP-статус и тело. */
    public static final class Response {
        /** HTTP-статус. */
        public final int status;
        /** Тело как строка. */
        public final String body;

        Response(int status, String body) {
            this.status = status;
            this.body = body;
        }

        /** Успешный статус (2xx). */
        public boolean ok() {
            return status >= 200 && status < 300;
        }
    }

    /** GET-запрос. */
    public static Response get(String url, String bearer) throws IOException {
        return request("GET", url, bearer, null);
    }

    /** POST-запрос с JSON-телом. */
    public static Response postJson(String url, String bearer, String json) throws IOException {
        return request("POST", url, bearer, json);
    }

    /**
     * POST-поток (большие файлы: модель эмбеддингов из assets APK).
     *
     * Тело читается поблочно (chunked), память не растёт с размером файла.
     */
    public static Response postStream(String url, String bearer, InputStream body, String contentType)
            throws IOException {
        HttpURLConnection connection = (HttpURLConnection) new URL(url).openConnection();
        try {
            connection.setRequestMethod("POST");
            connection.setConnectTimeout(CONNECT_TIMEOUT_MS);
            connection.setReadTimeout(UPLOAD_READ_TIMEOUT_MS);
            connection.setDoOutput(true);
            connection.setChunkedStreamingMode(64 * 1024);
            connection.setRequestProperty("Content-Type", contentType);
            if (bearer != null && !bearer.isEmpty()) {
                connection.setRequestProperty("Authorization", "Bearer " + bearer);
            }
            try (OutputStream out = connection.getOutputStream()) {
                byte[] chunk = new byte[64 * 1024];
                int read;
                while ((read = body.read(chunk)) > 0) {
                    out.write(chunk, 0, read);
                }
            }
            int status = connection.getResponseCode();
            InputStream stream = status >= 400 ? connection.getErrorStream()
                    : connection.getInputStream();
            String text = stream == null ? "" : readAll(stream);
            return new Response(status, text);
        } finally {
            connection.disconnect();
        }
    }

    /** DELETE-запрос. */
    public static Response delete(String url, String bearer) throws IOException {
        return request("DELETE", url, bearer, null);
    }

    /** Таймаут загрузки файла (модель 224 МБ по Wi-Fi). */
    private static final int UPLOAD_READ_TIMEOUT_MS = 600000;

    private static Response request(String method, String url, String bearer, String body)
            throws IOException {
        HttpURLConnection connection = (HttpURLConnection) new URL(url).openConnection();
        try {
            connection.setRequestMethod(method);
            connection.setConnectTimeout(CONNECT_TIMEOUT_MS);
            connection.setReadTimeout(READ_TIMEOUT_MS);
            connection.setRequestProperty("Accept", "application/json");
            if (bearer != null && !bearer.isEmpty()) {
                connection.setRequestProperty("Authorization", "Bearer " + bearer);
            }
            if (body != null) {
                connection.setDoOutput(true);
                connection.setRequestProperty("Content-Type", "application/json");
                byte[] bytes = body.getBytes(StandardCharsets.UTF_8);
                connection.setFixedLengthStreamingMode(bytes.length);
                try (OutputStream out = connection.getOutputStream()) {
                    out.write(bytes);
                }
            }

            int status = connection.getResponseCode();
            InputStream stream = status >= 400 ? connection.getErrorStream()
                    : connection.getInputStream();
            String text = stream == null ? "" : readAll(stream);
            return new Response(status, text);
        } finally {
            connection.disconnect();
        }
    }

    /** Прочитать поток целиком как UTF-8. */
    private static String readAll(InputStream stream) throws IOException {
        ByteArrayOutputStream buffer = new ByteArrayOutputStream();
        byte[] chunk = new byte[8192];
        int read;
        while ((read = stream.read(chunk)) > 0) {
            buffer.write(chunk, 0, read);
        }
        stream.close();
        return buffer.toString("UTF-8");
    }
}
