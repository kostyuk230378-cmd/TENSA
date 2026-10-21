package com.tensorlogix.tensa;

import android.content.Intent;
import android.os.Bundle;
import android.view.Gravity;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import org.json.JSONObject;

/**
 * Мобильный дом Tensa: состояние подключения к ПК и разделы секретаря.
 *
 * Модели и файлы живут на ПК; телефон — пульт. При первом запуске без
 * адреса сервера открывается {@link OnboardingActivity}.
 */
public class MainActivity extends android.app.Activity {

    /** Строка состояния подключения. */
    private TextView connectionText;
    /** Цветная точка состояния. */
    private View connectionDot;
    /** Адрес сервера/подсказка. */
    private TextView connectionHint;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        if (!Prefs.onboardingDone(this) && Prefs.server(this).isEmpty()) {
            startActivity(new Intent(this, OnboardingActivity.class));
            finish();
            return;
        }
        Ui.style(this);

        LinearLayout content = Ui.column(this);
        content.setPadding(Ui.dp(this, 20), Ui.dp(this, 36),
                Ui.dp(this, 20), Ui.dp(this, 28));

        LinearLayout header = Ui.row(this);
        header.addView(Ui.logo(this, 44));
        LinearLayout titleColumn = Ui.column(this);
        titleColumn.setPadding(Ui.dp(this, 12), 0, 0, 0);
        titleColumn.addView(Ui.text(this, "Tensa", 27, Ui.TEXT,
                android.graphics.Typeface.BOLD));
        titleColumn.addView(Ui.muted(this, "мобильный секретарь"));
        header.addView(titleColumn, new LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        TextView settings = Ui.text(this, "Настройки", 13, Ui.CYAN,
                android.graphics.Typeface.BOLD);
        settings.setPadding(Ui.dp(this, 8), Ui.dp(this, 8), 0, Ui.dp(this, 8));
        settings.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                startActivity(new Intent(MainActivity.this, SettingsActivity.class));
            }
        });
        header.addView(settings);
        content.addView(header);

        LinearLayout statusCard = Ui.card(this);
        statusCard.setClickable(true);
        statusCard.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                startActivity(new Intent(MainActivity.this, SettingsActivity.class));
            }
        });
        LinearLayout statusRow = Ui.row(this);
        connectionDot = new View(this);
        LinearLayout.LayoutParams dotParams = new LinearLayout.LayoutParams(
                Ui.dp(this, 9), Ui.dp(this, 9));
        dotParams.setMargins(0, 0, Ui.dp(this, 9), 0);
        connectionDot.setLayoutParams(dotParams);
        connectionDot.setBackground(Ui.box(Ui.MUTED, 0, Ui.dp(this, 5)));
        statusRow.addView(connectionDot);
        connectionText = Ui.text(this, "ПК не подключён", 15, Ui.TEXT,
                android.graphics.Typeface.BOLD);
        statusRow.addView(connectionText);
        statusCard.addView(statusRow);
        connectionHint = Ui.muted(this, "Укажите адрес сервера и токен в настройках.");
        connectionHint.setPadding(0, Ui.dp(this, 7), 0, 0);
        statusCard.addView(connectionHint);
        LinearLayout.LayoutParams statusParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        statusParams.topMargin = Ui.dp(this, 20);
        content.addView(statusCard, statusParams);

        LinearLayout chat = Ui.card(this);
        chat.setClickable(true);
        chat.setFocusable(true);
        chat.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                startActivity(new Intent(MainActivity.this, ChatActivity.class));
            }
        });
        TextView chatIcon = Ui.text(this, "●", 22, Ui.GREEN, android.graphics.Typeface.NORMAL);
        chat.addView(chatIcon);
        TextView chatTitle = Ui.text(this, "Спросить Tensa", 22, Ui.TEXT,
                android.graphics.Typeface.BOLD);
        chatTitle.setPadding(0, Ui.dp(this, 4), 0, 0);
        chat.addView(chatTitle);
        chat.addView(Ui.muted(this, "Текст или голос. Модель работает на ПК."));
        TextView chatAction = Ui.button(this, "Открыть чат", Ui.GREEN, Ui.BG);
        LinearLayout.LayoutParams chatActionParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        chatActionParams.topMargin = Ui.dp(this, 14);
        chat.addView(chatAction, chatActionParams);
        chatAction.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                startActivity(new Intent(MainActivity.this, ChatActivity.class));
            }
        });
        LinearLayout.LayoutParams chatParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        chatParams.topMargin = Ui.dp(this, 14);
        content.addView(chat, chatParams);

        content.addView(Ui.gap(this, 14));
        content.addView(Ui.menuCard(this, "⌕", "Файлы",
                "Найти документ на ПК", new View.OnClickListener() {
                    @Override
                    public void onClick(View view) {
                        startActivity(new Intent(MainActivity.this, FilesActivity.class));
                    }
                }));
        content.addView(Ui.gap(this, 10));
        content.addView(Ui.menuCard(this, "✉", "Почта",
                "Аккаунты и отправка через Tensa", new View.OnClickListener() {
                    @Override
                    public void onClick(View view) {
                        startActivity(new Intent(MainActivity.this, EmailActivity.class));
                    }
                }));
        content.addView(Ui.gap(this, 10));
        content.addView(Ui.menuCard(this, "✓", "Одобрения",
                "Действия, которые ждут вашего решения", new View.OnClickListener() {
                    @Override
                    public void onClick(View view) {
                        startActivity(new Intent(MainActivity.this, ApprovalsActivity.class));
                    }
                }));
        content.addView(Ui.gap(this, 10));
        content.addView(Ui.menuCard(this, "≡", "Заметки и поиск",
                "Личные заметки и поиск по ним", new View.OnClickListener() {
                    @Override
                    public void onClick(View view) {
                        startActivity(new Intent(MainActivity.this, NotesActivity.class));
                    }
                }));
        content.addView(Ui.gap(this, 10));
        content.addView(Ui.menuCard(this, "▦", "Календарь",
                "Встречи и события", new View.OnClickListener() {
                    @Override
                    public void onClick(View view) {
                        startActivity(new Intent(MainActivity.this, CalendarActivity.class));
                    }
                }));
        content.addView(Ui.gap(this, 10));
        content.addView(Ui.menuCard(this, "⌂", "Плавающий виджет",
                "Быстрый доступ поверх приложений", new View.OnClickListener() {
                    @Override
                    public void onClick(View view) {
                        startActivity(new Intent(MainActivity.this, SettingsActivity.class));
                    }
                }));

        TextView note = Ui.muted(this,
                "Модели, документы и почта не покидают ваш ПК без вашего решения. "
                        + "Ключи не предустановлены — всё вводится вами.");
        note.setTextSize(11);
        note.setLineSpacing(0, 1.2f);
        note.setPadding(0, Ui.dp(this, 18), 0, 0);
        content.addView(note);

        ScrollView scroll = new ScrollView(this);
        scroll.setBackgroundColor(Ui.BG);
        scroll.addView(content);
        setContentView(scroll);
    }

    @Override
    protected void onResume() {
        super.onResume();
        refreshConnection();
    }

    /** Проверить ядро и обновить верхнюю карточку. */
    private void refreshConnection() {
        final String server = Prefs.server(this);
        final String bearer = Prefs.coreToken(this);
        if (server.isEmpty() || bearer.isEmpty()) {
            connectionDot.setBackground(Ui.box(Ui.MUTED, 0, Ui.dp(this, 5)));
            connectionText.setText("ПК не подключён");
            connectionHint.setText("Укажите адрес сервера и токен в настройках.");
            return;
        }
        connectionText.setText("Проверяю связь…");
        connectionHint.setText(server);
        Background.run(new Background.Task<JSONObject>() {
            @Override
            public JSONObject run() throws Exception {
                return CoreClient.health(server, bearer);
            }
        }, new Background.Callback<JSONObject>() {
            @Override
            public void onResult(JSONObject value) {
                boolean healthy = value.optBoolean("healthy", true);
                connectionDot.setBackground(Ui.box(healthy ? Ui.GREEN : Ui.RED, 0, Ui.dp(MainActivity.this, 5)));
                connectionText.setText(healthy ? "ПК на связи" : "Ядро деградировало");
                connectionHint.setText(server);
            }

            @Override
            public void onError(Exception error) {
                connectionDot.setBackground(Ui.box(Ui.RED, 0, Ui.dp(MainActivity.this, 5)));
                connectionText.setText("Нет связи с ПК");
                connectionHint.setText(server + " · " + error.getMessage());
            }
        });
    }
}
