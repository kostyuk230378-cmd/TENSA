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
 * Заметки: создание и список (ядро, Контракт 3).
 */
public class NotesActivity extends android.app.Activity {

    /** Контейнер списка заметок. */
    private LinearLayout list;
    /** Строка состояния. */
    private TextView status;
    /** Поле заголовка. */
    private EditText title;
    /** Поле текста. */
    private EditText body;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(20, 20, 20, 12);

        TextView caption = new TextView(this);
        caption.setText("Заметки");
        caption.setTextSize(22);
        root.addView(caption);

        title = new EditText(this);
        title.setHint("Заголовок");
        root.addView(title);

        body = new EditText(this);
        body.setHint("Текст");
        body.setMaxLines(3);
        root.addView(body);

        LinearLayout actions = new LinearLayout(this);
        actions.setOrientation(LinearLayout.HORIZONTAL);

        Button save = new Button(this);
        save.setText("Сохранить");
        save.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                create();
            }
        });
        actions.addView(save);

        Button reload = new Button(this);
        reload.setText("Обновить");
        reload.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                load();
            }
        });
        actions.addView(reload);
        root.addView(actions);

        status = new TextView(this);
        status.setPadding(0, 6, 0, 8);
        root.addView(status);

        list = new LinearLayout(this);
        list.setOrientation(LinearLayout.VERTICAL);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(list);
        root.addView(scroll, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));

        setContentView(root);
        load();
    }

    /** Создать заметку из формы. */
    private void create() {
        final String noteTitle = title.getText().toString().trim();
        final String noteBody = body.getText().toString().trim();
        if (noteTitle.isEmpty()) {
            status.setText("Нужен заголовок.");
            return;
        }
        final String server = Prefs.server(this);
        final String token = Prefs.coreToken(this);

        Background.run(new Background.Task<Void>() {
            @Override
            public Void run() throws Exception {
                CoreClient.createNote(server, token, noteTitle, noteBody);
                return null;
            }
        }, new Background.Callback<Void>() {
            @Override
            public void onResult(Void value) {
                title.setText("");
                body.setText("");
                status.setText("Сохранено.");
                load();
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка создания: " + error.getMessage());
            }
        });
    }

    /** Загрузить заметки. */
    private void load() {
        final String server = Prefs.server(this);
        final String token = Prefs.coreToken(this);
        Background.run(new Background.Task<JSONArray>() {
            @Override
            public JSONArray run() throws Exception {
                return CoreClient.notes(server, token);
            }
        }, new Background.Callback<JSONArray>() {
            @Override
            public void onResult(JSONArray notes) {
                list.removeAllViews();
                status.setText(notes.length() == 0 ? "Заметок нет." : "");
                for (int i = 0; i < notes.length(); i++) {
                    addNote(notes.optJSONObject(i));
                }
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка: " + error.getMessage());
            }
        });
    }

    /** Строка заметки с кнопкой удаления. */
    private void addNote(final JSONObject note) {
        if (note == null) {
            return;
        }
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setPadding(0, 10, 0, 10);

        TextView text = new TextView(this);
        String noteTitle = note.optString("title", "?");
        String noteBody = note.optString("body", "");
        if (noteBody.length() > 160) {
            noteBody = noteBody.substring(0, 160) + "…";
        }
        text.setText(noteTitle + (noteBody.isEmpty() ? "" : "\n" + noteBody));
        card.addView(text);

        final String id = note.optString("id", "");
        Button remove = new Button(this);
        remove.setText("Удалить");
        remove.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                delete(id);
            }
        });
        card.addView(remove);
        list.addView(card);
    }

    /** Удалить заметку и обновить список. */
    private void delete(final String id) {
        final String server = Prefs.server(this);
        final String token = Prefs.coreToken(this);
        Background.run(new Background.Task<Void>() {
            @Override
            public Void run() throws Exception {
                CoreClient.deleteNote(server, token, id);
                return null;
            }
        }, new Background.Callback<Void>() {
            @Override
            public void onResult(Void value) {
                load();
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка удаления: " + error.getMessage());
            }
        });
    }
}
