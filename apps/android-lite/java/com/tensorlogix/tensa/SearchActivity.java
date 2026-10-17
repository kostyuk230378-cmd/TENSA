package com.tensorlogix.tensa;

import android.os.Bundle;
import android.view.View;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Поиск по заметкам: `GET /api/v1/notes/search` (FTS5 ядра).
 */
public class SearchActivity extends android.app.Activity {

    /** Поле запроса. */
    private EditText query;
    /** Строка состояния. */
    private TextView status;
    /** Контейнер результатов. */
    private LinearLayout results;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(20, 20, 20, 12);

        TextView title = new TextView(this);
        title.setText("Поиск по заметкам");
        title.setTextSize(22);
        root.addView(title);

        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);

        query = new EditText(this);
        query.setHint("Запрос…");
        row.addView(query, new LinearLayout.LayoutParams(
                0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));

        Button search = new Button(this);
        search.setText("Найти");
        search.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                search();
            }
        });
        row.addView(search);
        root.addView(row);

        status = new TextView(this);
        status.setPadding(0, 6, 0, 8);
        root.addView(status);

        results = new LinearLayout(this);
        results.setOrientation(LinearLayout.VERTICAL);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(results);
        root.addView(scroll, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));

        setContentView(root);
    }

    /** Выполнить поиск по ядру. */
    private void search() {
        final String text = query.getText().toString().trim();
        if (text.isEmpty()) {
            status.setText("Введите запрос.");
            return;
        }
        status.setText("Ищу…");
        results.removeAllViews();
        final String server = Prefs.server(this);
        final String bearer = Prefs.coreToken(this);

        Background.run(new Background.Task<JSONArray>() {
            @Override
            public JSONArray run() throws Exception {
                return CoreClient.searchNotes(server, bearer, text, 20);
            }
        }, new Background.Callback<JSONArray>() {
            @Override
            public void onResult(JSONArray notes) {
                status.setText(notes.length() == 0 ? "Ничего не найдено." : "");
                for (int i = 0; i < notes.length(); i++) {
                    JSONObject note = notes.optJSONObject(i);
                    if (note == null) {
                        continue;
                    }
                    TextView item = new TextView(SearchActivity.this);
                    String noteTitle = note.optString("title", "?");
                    String body = note.optString("body", "");
                    if (body.length() > 160) {
                        body = body.substring(0, 160) + "…";
                    }
                    item.setText(noteTitle + (body.isEmpty() ? "" : "\n" + body));
                    item.setTextSize(14);
                    item.setPadding(0, 10, 0, 10);
                    results.addView(item);
                }
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка: " + error.getMessage());
            }
        });
    }
}
