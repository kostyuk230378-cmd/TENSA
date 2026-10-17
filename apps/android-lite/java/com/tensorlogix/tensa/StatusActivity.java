package com.tensorlogix.tensa;

import android.os.Bundle;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

import org.json.JSONObject;

/**
 * Статус ядра: /api/v1/health на экране.
 */
public class StatusActivity extends android.app.Activity {

    /** Текст статуса. */
    private TextView output;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(20, 20, 20, 12);

        TextView title = new TextView(this);
        title.setText("Статус ядра");
        title.setTextSize(22);
        root.addView(title);

        Button reload = new Button(this);
        reload.setText("Обновить");
        reload.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                load();
            }
        });
        root.addView(reload);

        output = new TextView(this);
        output.setTextSize(14);
        output.setText("Загрузка…");
        root.addView(output);

        setContentView(root);
        load();
    }

    /** Запросить здоровье ядра и показать текстом. */
    private void load() {
        final String server = Prefs.server(this);
        final String token = Prefs.coreToken(this);
        output.setText("Загрузка…");

        Background.run(new Background.Task<String>() {
            @Override
            public String run() throws Exception {
                JSONObject health = CoreClient.health(server, token);
                return "Ядро: " + (health.optBoolean("healthy") ? "здоров" : "деградация")
                        + "\nВерсия: " + health.optString("version")
                        + "\nАптайм: " + health.optString("uptime")
                        + "\nСервер: " + server;
            }
        }, new Background.Callback<String>() {
            @Override
            public void onResult(String value) {
                output.setText(value);
            }

            @Override
            public void onError(Exception error) {
                output.setText("Ошибка: " + error.getMessage());
            }
        });
    }
}
