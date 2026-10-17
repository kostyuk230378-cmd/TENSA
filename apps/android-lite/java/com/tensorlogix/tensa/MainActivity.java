package com.tensorlogix.tensa;

import android.content.Intent;
import android.os.Bundle;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

/**
 * Главный экран: меню разделов Tensa и состояние подключений.
 *
 * Экраны: Чат (большая модель через OpenRouter), Одобрения, Заметки,
 * Статус ядра, Настройки.
 */
public class MainActivity extends android.app.Activity {

    /** Подпись состояния подключений (обновляется в onResume). */
    private TextView status;

    /** Кнопка раздела. */
    private Button menuButton(String title, final Class<?> target) {
        Button button = new Button(this);
        button.setText(title);
        button.setTextSize(17);
        button.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                startActivity(new Intent(MainActivity.this, target));
            }
        });
        return button;
    }

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(32, 48, 32, 32);
        root.setGravity(Gravity.CENTER_HORIZONTAL);

        TextView title = new TextView(this);
        title.setText("Tensa");
        title.setTextSize(34);
        title.setGravity(Gravity.CENTER);
        root.addView(title);

        status = new TextView(this);
        status.setTextSize(13);
        status.setGravity(Gravity.CENTER);
        status.setPadding(0, 8, 0, 28);
        root.addView(status);

        root.addView(menuButton("Чат (OpenRouter)", ChatActivity.class));
        root.addView(menuButton("Одобрения", ApprovalsActivity.class));
        root.addView(menuButton("Заметки", NotesActivity.class));
        root.addView(menuButton("Поиск", SearchActivity.class));
        root.addView(menuButton("Почта", EmailActivity.class));
        root.addView(menuButton("Календарь", CalendarActivity.class));
        root.addView(menuButton("Статус ядра", StatusActivity.class));
        root.addView(menuButton("Настройки", SettingsActivity.class));

        ScrollView scroll = new ScrollView(this);
        scroll.addView(root);
        setContentView(scroll);
    }

    @Override
    protected void onResume() {
        super.onResume();
        boolean core = !Prefs.coreToken(this).isEmpty();
        boolean cloud = !Prefs.openRouterKey(this).isEmpty();
        String server = Prefs.server(this);
        status.setText("Ядро: " + (server.isEmpty() ? "не задан" : server)
                + (core ? "  ·  токен ✓" : "  ·  токен —")
                + "\nOpenRouter: " + Prefs.model(this) + (cloud ? "  ·  ключ ✓" : "  ·  ключ —"));
    }
}
