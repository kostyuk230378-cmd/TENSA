package com.tensorlogix.tensa;

import android.content.Intent;
import android.os.Bundle;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import org.json.JSONObject;

/**
 * Первый запуск: подключение телефона к ядру Tensa на ПК.
 *
 * Никаких ключей и моделей заранее нет: пользователь вводит адрес сервера,
 * который показан в настройках Tensa на ПК, и токен из файла pwa_token.
 */
public class OnboardingActivity extends android.app.Activity {

    /** Поле адреса. */
    private EditText serverField;
    /** Поле токена. */
    private EditText tokenField;
    /** Статус проверки. */
    private TextView status;
    /** Кнопка проверки. */
    private TextView connectButton;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Ui.style(this);

        LinearLayout content = Ui.column(this);
        content.setPadding(Ui.dp(this, 24), Ui.dp(this, 48),
                Ui.dp(this, 24), Ui.dp(this, 24));

        LinearLayout brand = Ui.row(this);
        brand.setGravity(Gravity.CENTER_VERTICAL);
        brand.addView(Ui.logo(this, 64));
        LinearLayout brandText = Ui.column(this);
        brandText.setPadding(Ui.dp(this, 14), 0, 0, 0);
        TextView name = Ui.text(this, "Tensa", 32, Ui.TEXT, android.graphics.Typeface.BOLD);
        brandText.addView(name);
        brandText.addView(Ui.muted(this, "мобильный секретарь"));
        brand.addView(brandText);
        content.addView(brand);

        TextView lead = Ui.text(this, "Подключим ваш ПК", 24, Ui.TEXT,
                android.graphics.Typeface.BOLD);
        lead.setPadding(0, Ui.dp(this, 30), 0, Ui.dp(this, 8));
        content.addView(lead);

        TextView explain = Ui.muted(this,
                "Модели, файлы и почта остаются на компьютере. Телефон — это пульт. "
                        + "Адрес сервера показан в настройках Tensa на ПК, над карточкой «Микрофон».");
        explain.setLineSpacing(0, 1.25f);
        content.addView(explain);

        LinearLayout card = Ui.card(this);
        LinearLayout.LayoutParams cardParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        cardParams.topMargin = Ui.dp(this, 20);

        card.addView(Ui.text(this, "Адрес сервера", 13, Ui.MUTED,
                android.graphics.Typeface.BOLD));
        serverField = Ui.field(this, "http://<ip-пк>:8765", Prefs.server(this),
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        serverField.setSingleLine(true);
        serverField.setPadding(Ui.dp(this, 14), Ui.dp(this, 12),
                Ui.dp(this, 14), Ui.dp(this, 12));
        LinearLayout.LayoutParams fieldParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        fieldParams.topMargin = Ui.dp(this, 6);
        card.addView(serverField, fieldParams);

        TextView tokenLabel = Ui.text(this, "Токен ядра", 13, Ui.MUTED,
                android.graphics.Typeface.BOLD);
        tokenLabel.setPadding(0, Ui.dp(this, 16), 0, 0);
        card.addView(tokenLabel);
        tokenField = Ui.field(this, "файл pwa_token", Prefs.coreToken(this),
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        tokenField.setSingleLine(true);
        tokenField.setPadding(Ui.dp(this, 14), Ui.dp(this, 12),
                Ui.dp(this, 14), Ui.dp(this, 12));
        card.addView(tokenField, fieldParams);

        TextView hint = Ui.muted(this, "Токен лежит в %USERPROFILE%\\.tensa\\pwa_token");
        hint.setTextSize(11);
        hint.setPadding(0, Ui.dp(this, 8), 0, 0);
        card.addView(hint);

        status = Ui.muted(this, "");
        status.setPadding(0, Ui.dp(this, 12), 0, 0);
        card.addView(status);

        connectButton = Ui.button(this, "Проверить и подключиться", Ui.GREEN, Ui.BG);
        LinearLayout.LayoutParams buttonParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        buttonParams.topMargin = Ui.dp(this, 14);
        card.addView(connectButton, buttonParams);
        connectButton.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                connect();
            }
        });

        TextView skip = Ui.button(this, "Настрою позже", Ui.CARD, Ui.CYAN);
        LinearLayout.LayoutParams skipParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        skipParams.topMargin = Ui.dp(this, 10);
        card.addView(skip, skipParams);
        skip.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                Prefs.setOnboardingDone(OnboardingActivity.this, true);
                openMain();
            }
        });

        content.addView(card, cardParams);

        TextView footnote = Ui.muted(this,
                "Ключ OpenRouter не обязателен. Если он понадобится, его можно ввести "
                        + "позже в Настройках — приложение не использует чужие ключи.");
        footnote.setTextSize(11);
        footnote.setPadding(0, Ui.dp(this, 18), 0, 0);
        footnote.setLineSpacing(0, 1.2f);
        content.addView(footnote);

        ScrollView scroll = new ScrollView(this);
        scroll.setBackgroundColor(Ui.BG);
        scroll.addView(content);
        setContentView(scroll);
    }

    /** Проверить подключение и сохранить адрес/токен. */
    private void connect() {
        final String server = Prefs.normalize(serverField.getText().toString());
        final String bearer = tokenField.getText().toString().trim();
        if (server.isEmpty()) {
            status.setTextColor(Ui.RED);
            status.setText("Введите адрес сервера.");
            return;
        }
        if (!server.startsWith("http://") && !server.startsWith("https://")) {
            status.setTextColor(Ui.RED);
            status.setText("Адрес должен начинаться с http://");
            return;
        }
        if (bearer.isEmpty()) {
            status.setTextColor(Ui.RED);
            status.setText("Введите токен из файла pwa_token.");
            return;
        }
        connectButton.setEnabled(false);
        connectButton.setText("Проверяю…");
        status.setTextColor(Ui.MUTED);
        status.setText("Соединяюсь с ядром Tensa…");

        Background.run(new Background.Task<String>() {
            @Override
            public String run() throws Exception {
                JSONObject health = CoreClient.health(server, bearer);
                return "Tensa " + health.optString("version", "ядро") + " на связи";
            }
        }, new Background.Callback<String>() {
            @Override
            public void onResult(String value) {
                Prefs.saveConnection(OnboardingActivity.this, server, bearer);
                status.setTextColor(Ui.GREEN);
                status.setText(value);
                openMain();
            }

            @Override
            public void onError(Exception error) {
                connectButton.setEnabled(true);
                connectButton.setText("Проверить и подключиться");
                status.setTextColor(Ui.RED);
                status.setText("Не удалось: " + error.getMessage());
            }
        });
    }

    private void openMain() {
        startActivity(new Intent(this, MainActivity.class));
        finish();
    }
}
