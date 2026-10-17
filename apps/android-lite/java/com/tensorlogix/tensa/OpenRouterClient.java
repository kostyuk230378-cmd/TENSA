package com.tensorlogix.tensa;

import java.io.IOException;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Клиент большой модели через OpenRouter (chat completions).
 *
 * Запрос идёт напрямую с телефона: ключ хранится в настройках приложения.
 * Ответ читается из `choices[0].message.content`.
 */
public final class OpenRouterClient {

    /** Адрес chat completions OpenRouter. */
    private static final String URL = "https://openrouter.ai/api/v1/chat/completions";

    /** Системная инструкция секретаря. */
    private static final String SYSTEM = "Ты — Tensa, личный секретарь пользователя. "
            + "Отвечай кратко, по делу, на русском языке. Если нужно действие "
            + "(письмо, заметка, событие) — предложи его и параметры.";

    private OpenRouterClient() {
    }

    /**
     * Отправить вопрос модели и вернуть текст ответа.
     *
     * @param key ключ OpenRouter
     * @param model имя модели
     * @param userText вопрос пользователя
     * @return текст ответа
     * @throws Exception сеть/API/разбор
     */
    public static String chat(String key, String model, String userText) throws Exception {
        if (key == null || key.trim().isEmpty()) {
            throw new IllegalStateException("Не задан ключ OpenRouter — откройте Настройки");
        }

        JSONArray messages = new JSONArray();
        messages.put(new JSONObject().put("role", "system").put("content", SYSTEM));
        messages.put(new JSONObject().put("role", "user").put("content", userText));

        JSONObject request = new JSONObject();
        request.put("model", model);
        request.put("messages", messages);

        Http.Response response = Http.postJson(URL, key.trim(), request.toString());
        if (response.status == 401) {
            throw new IOException("OpenRouter: неверный ключ (401)");
        }
        if (!response.ok()) {
            throw new IOException("OpenRouter HTTP " + response.status + ": " + shorten(response.body));
        }

        JSONObject json = new JSONObject(response.body);
        JSONArray choices = json.optJSONArray("choices");
        if (choices == null || choices.length() == 0) {
            throw new IOException("OpenRouter: пустой ответ без choices");
        }
        JSONObject message = choices.getJSONObject(0).optJSONObject("message");
        String content = message == null ? null : message.optString("content", "");
        if (content == null || content.isEmpty()) {
            throw new IOException("OpenRouter: пустой текст ответа");
        }
        return content;
    }

    /** Укоротить тело ошибки для показа. */
    private static String shorten(String text) {
        String value = text == null ? "" : text.trim();
        return value.length() > 300 ? value.substring(0, 300) + "…" : value;
    }
}
