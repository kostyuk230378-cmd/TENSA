package com.tensorlogix.tensa;

import android.content.Context;
import android.content.SharedPreferences;

/**
 * Локальные настройки приложения (SharedPreferences).
 *
 * Хранятся: адрес сервера ядра Tensa и его PWA-токен, ключ OpenRouter,
 * имя модели и последний чат. Секреты не логируются; для продакшена —
 * EncryptedSharedPreferences/Android Keystore (этап hardening).
 */
public final class Prefs {

    /** Имя файла настроек. */
    private static final String FILE = "tensa";
    /** Адрес сервера ядра (задаётся пользователем в Настройках). */
    private static final String KEY_SERVER = "server";
    /** Bearer-токен API ядра (файл pwa_token на ПК). */
    private static final String KEY_CORE_TOKEN = "core_token";
    /** Ключ OpenRouter (sk-or-...). */
    private static final String KEY_OPENROUTER = "openrouter_key";
    /** Модель OpenRouter. */
    private static final String KEY_MODEL = "openrouter_model";

    /** Адрес сервера по умолчанию (ПК в локальной сети). */
    public static final String DEFAULT_SERVER = "";
    /** Модель OpenRouter по умолчанию. */
    public static final String DEFAULT_MODEL = "openai/gpt-4o-mini";

    private Prefs() {
    }

    private static SharedPreferences prefs(Context context) {
        return context.getSharedPreferences(FILE, Context.MODE_PRIVATE);
    }

    /** Адрес сервера ядра. */
    public static String server(Context context) {
        return prefs(context).getString(KEY_SERVER, DEFAULT_SERVER);
    }

    /** Токен API ядра. */
    public static String coreToken(Context context) {
        return prefs(context).getString(KEY_CORE_TOKEN, "");
    }

    /** Ключ OpenRouter. */
    public static String openRouterKey(Context context) {
        return prefs(context).getString(KEY_OPENROUTER, "");
    }

    /** Модель OpenRouter. */
    public static String model(Context context) {
        return prefs(context).getString(KEY_MODEL, DEFAULT_MODEL);
    }

    /** Сохранить настройки. */
    public static void save(Context context, String server, String coreToken,
                            String openRouterKey, String model) {
        prefs(context).edit()
                .putString(KEY_SERVER, normalize(server))
                .putString(KEY_CORE_TOKEN, coreToken.trim())
                .putString(KEY_OPENROUTER, openRouterKey.trim())
                .putString(KEY_MODEL, model.trim().isEmpty() ? DEFAULT_MODEL : model.trim())
                .apply();
    }

    /** Убрать хвостовые слэши адреса. */
    private static String normalize(String raw) {
        String value = raw == null ? "" : raw.trim();
        while (value.endsWith("/")) {
            value = value.substring(0, value.length() - 1);
        }
        return value;
    }
}
