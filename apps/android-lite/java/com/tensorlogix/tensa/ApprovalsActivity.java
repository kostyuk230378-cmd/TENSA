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
 * Очередь одобрения ядра: список действий с кнопками решения.
 *
 * Данные — GET /api/v1/approvals; решения — approve/reject с Bearer-токеном.
 */
public class ApprovalsActivity extends android.app.Activity {

    /** Контейнер карточек действий. */
    private LinearLayout list;
    /** Строка состояния (загрузка/ошибка). */
    private TextView status;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(20, 20, 20, 12);

        TextView title = new TextView(this);
        title.setText("Одобрения");
        title.setTextSize(22);
        root.addView(title);

        status = new TextView(this);
        status.setPadding(0, 6, 0, 8);
        root.addView(status);

        Button reload = new Button(this);
        reload.setText("Обновить");
        reload.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                load();
            }
        });
        root.addView(reload);

        list = new LinearLayout(this);
        list.setOrientation(LinearLayout.VERTICAL);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(list);
        root.addView(scroll, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));

        setContentView(root);
        load();
    }

    /** Загрузить очередь и перерисовать карточки. */
    private void load() {
        status.setText("Загрузка…");
        list.removeAllViews();
        final String server = Prefs.server(this);
        final String token = Prefs.coreToken(this);

        Background.run(new Background.Task<JSONArray>() {
            @Override
            public JSONArray run() throws Exception {
                return CoreClient.approvals(server, token);
            }
        }, new Background.Callback<JSONArray>() {
            @Override
            public void onResult(JSONArray actions) {
                status.setText(actions.length() == 0 ? "Ожидающих решений нет." : "");
                for (int i = 0; i < actions.length(); i++) {
                    addCard(actions.optJSONObject(i));
                }
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка: " + error.getMessage());
            }
        });
    }

    /** Карточка одного действия. */
    private void addCard(final JSONObject action) {
        if (action == null) {
            return;
        }
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setPadding(0, 12, 0, 12);

        TextView caption = new TextView(this);
        String kind = action.optString("kind", "?");
        String state = action.optString("status", "?");
        String payload = action.opt("payload") == null ? "" : action.opt("payload").toString();
        if (payload.length() > 220) {
            payload = payload.substring(0, 220) + "…";
        }
        caption.setText(kind + " · " + state + "\n" + payload);
        caption.setTextSize(14);
        card.addView(caption);

        final String id = action.optString("id", "");
        if ("pending".equals(state) && !id.isEmpty()) {
            LinearLayout row = new LinearLayout(this);
            row.setOrientation(LinearLayout.HORIZONTAL);

            Button approve = new Button(this);
            approve.setText("✔ Подтвердить");
            approve.setOnClickListener(new View.OnClickListener() {
                @Override
                public void onClick(View view) {
                    decide(id, true);
                }
            });
            row.addView(approve);

            Button reject = new Button(this);
            reject.setText("✖ Отклонить");
            reject.setOnClickListener(new View.OnClickListener() {
                @Override
                public void onClick(View view) {
                    decide(id, false);
                }
            });
            row.addView(reject);
            card.addView(row);
        }

        list.addView(card);
    }

    /** Отправить решение и перезагрузить список. */
    private void decide(final String id, final boolean approve) {
        final String server = Prefs.server(this);
        final String token = Prefs.coreToken(this);
        Background.run(new Background.Task<Void>() {
            @Override
            public Void run() throws Exception {
                if (approve) {
                    CoreClient.approve(server, token, id);
                } else {
                    CoreClient.reject(server, token, id);
                }
                return null;
            }
        }, new Background.Callback<Void>() {
            @Override
            public void onResult(Void value) {
                load();
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка решения: " + error.getMessage());
            }
        });
    }
}
