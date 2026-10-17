package com.tensorlogix.tensa;

import android.os.Bundle;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Почта: состав аккаунтов из конфига ядра (`GET /api/v1/email/accounts`).
 *
 * Отправка и чтение писем выполняются контуром tensa-tools (SMTP/IMAP,
 * подтверждение действия через очередь одобрения); здесь — реальные
 * аккаунты ядра, чтобы экран не был пустым.
 */
public class EmailActivity extends android.app.Activity {

    /** Строка состояния. */
    private TextView status;
    /** Контейнер аккаунтов. */
    private LinearLayout list;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(20, 20, 20, 12);

        TextView title = new TextView(this);
        title.setText("Почта");
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

        status = new TextView(this);
        status.setPadding(0, 6, 0, 8);
        root.addView(status);

        list = new LinearLayout(this);
        list.setOrientation(LinearLayout.VERTICAL);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(list);
        root.addView(scroll, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));

        TextView hint = new TextView(this);
        hint.setTextSize(12);
        hint.setText("Отправка писем — через очередь одобрения (бот/CLI): письмо\n"
                + "создаётся инструментом email, подтверждается и уходит по SMTP.");
        root.addView(hint);

        setContentView(root);
        load();
    }

    /** Загрузить аккаунты. */
    private void load() {
        status.setText("Загрузка…");
        list.removeAllViews();
        final String server = Prefs.server(this);
        final String bearer = Prefs.coreToken(this);

        Background.run(new Background.Task<JSONArray>() {
            @Override
            public JSONArray run() throws Exception {
                return CoreClient.emailAccounts(server, bearer);
            }
        }, new Background.Callback<JSONArray>() {
            @Override
            public void onResult(JSONArray accounts) {
                status.setText(accounts.length() == 0 ? "Аккаунтов нет — добавьте [email.accounts] в конфиг." : "");
                for (int i = 0; i < accounts.length(); i++) {
                    JSONObject account = accounts.optJSONObject(i);
                    if (account == null) {
                        continue;
                    }
                    TextView item = new TextView(EmailActivity.this);
                    String label = account.optString("label", "");
                    String email = account.optString("email", "");
                    item.setText((label.isEmpty() ? account.optString("id") : label) + "\n" + email);
                    item.setTextSize(16);
                    item.setPadding(0, 12, 0, 12);
                    list.addView(item);
                }
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка: " + error.getMessage());
            }
        });
    }
}
