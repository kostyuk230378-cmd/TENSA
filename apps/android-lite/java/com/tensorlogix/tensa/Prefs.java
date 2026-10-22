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
    private static final String KEY_CORE_AUTH = "core_auth";
    /** Ключ OpenRouter (sk-or-...). */
    private static final String KEY_OPENROUTER = "openrouter_key";
    /** Модель OpenRouter. */
    private static final String KEY_MODEL = "openrouter_model";
    /** Пользователь уже видел первый экран. */
    private static final String KEY_ONBOARDING = "onboarding_done";
    /** Overlay-виджет включён пользователем. */
    private static final String KEY_OVERLAY = "overlay_enabled";
    /** Системный запрос добавления home-screen виджета уже показан. */
    private static final String KEY_WIDGET_ASKED = "widget_asked";
    /** Запрос разрешения «поверх приложений» уже показан. */
    private static final String KEY_OVERLAY_ASKED = "overlay_asked";
    /** Выбранный пользователем микрофон Android (AudioDeviceInfo id). */
    private static final String KEY_MIC_ID = "mic_id";
    /** Имя выбранного микрофона для показа без повторного опроса. */
    private static final String KEY_MIC_NAME = "mic_name";

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
        return prefs(context).getString(KEY_CORE_AUTH, "");
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
                .putString(KEY_CORE_AUTH, coreToken.trim())
                .putString(KEY_OPENROUTER, openRouterKey.trim())
                .putString(KEY_MODEL, model.trim().isEmpty() ? DEFAULT_MODEL : model.trim())
                .putBoolean(KEY_ONBOARDING, true)
                .apply();
    }

    /** Сохранить только подключение к ядру, не трогая ключи OpenRouter. */
    public static void saveConnection(Context context, String server, String coreToken) {
        prefs(context).edit()
                .putString(KEY_SERVER, normalize(server))
                .putString(KEY_CORE_AUTH, coreToken.trim())
                .putBoolean(KEY_ONBOARDING, true)
                .apply();
    }

    /** Показывать ли первый экран подключения. */
    public static boolean onboardingDone(Context context) {
        return prefs(context).getBoolean(KEY_ONBOARDING, false);
    }

    /** Отметить первый экран пройденным. */
    public static void setOnboardingDone(Context context, boolean value) {
        prefs(context).edit().putBoolean(KEY_ONBOARDING, value).apply();
    }

    /** Включён ли overlay-виджет (по умолчанию да). */
    public static boolean overlayEnabled(Context context) {
        return prefs(context).getBoolean(KEY_OVERLAY, true);
    }

    /** Запомнить состояние overlay. */
    public static void setOverlayEnabled(Context context, boolean value) {
        prefs(context).edit().putBoolean(KEY_OVERLAY, value).apply();
    }

    /** Показывали ли системный запрос home-screen виджета. */
    public static boolean widgetAsked(Context context) {
        return prefs(context).getBoolean(KEY_WIDGET_ASKED, false);
    }

    /** Отметить показ запроса home-screen виджета. */
    public static void setWidgetAsked(Context context, boolean value) {
        prefs(context).edit().putBoolean(KEY_WIDGET_ASKED, value).apply();
    }

    /** Показывали ли системный запрос разрешения overlay. */
    public static boolean overlayAsked(Context context) {
        return prefs(context).getBoolean(KEY_OVERLAY_ASKED, false);
    }

    /** Отметить показ запроса разрешения overlay. */
    public static void setOverlayAsked(Context context, boolean value) {
        prefs(context).edit().putBoolean(KEY_OVERLAY_ASKED, value).apply();
    }

    /** Id выбранного микрофона; 0 — системный по умолчанию. */
    public static int micId(Context context) {
        return prefs(context).getInt(KEY_MIC_ID, 0);
    }

    /** Имя выбранного микрофона (для подписи в настройках). */
    public static String micName(Context context) {
        return prefs(context).getString(KEY_MIC_NAME, "");
    }

    /** Запомнить микрофон; 0 сбрасывает на системный. */
    public static void setMic(Context context, int id, String name) {
        prefs(context).edit()
                .putInt(KEY_MIC_ID, id)
                .putString(KEY_MIC_NAME, id == 0 ? "" : (name == null ? "" : name))
                .apply();
    }

    /** Убрать хвостовые слэши адреса. */
    public static String normalize(String raw) {
        String value = raw == null ? "" : raw.trim();
        while (value.endsWith("/")) {
            value = value.substring(0, value.length() - 1);
        }
        return value;
    }
}
