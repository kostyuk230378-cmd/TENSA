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

import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.Locale;

/**
 * Календарь: список событий ядра и создание с guard'ом конфликтов.
 *
 * `GET/POST /api/v1/calendar/events`; перекрытие → 409, текст конфликта
 * показывается как есть («пересечение с: …»).
 */
public class CalendarActivity extends android.app.Activity {

    /** Формат ввода начала события. */
    private static final String INPUT_FORMAT = "yyyy-MM-dd HH:mm";

    /** Строка состояния. */
    private TextView status;
    /** Контейнер событий. */
    private LinearLayout list;
    /** Заголовок нового события. */
    private EditText titleField;
    /** Начало («2026-10-18 15:00»). */
    private EditText startField;
    /** Длительность в минутах. */
    private EditText durationField;
    /** Место. */
    private EditText locationField;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(20, 20, 20, 12);

        TextView header = new TextView(this);
        header.setText("Календарь");
        header.setTextSize(22);
        root.addView(header);

        titleField = new EditText(this);
        titleField.setHint("Название события");
        root.addView(titleField);

        startField = new EditText(this);
        startField.setHint("Начало: 2026-10-18 15:00");
        root.addView(startField);

        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);

        durationField = new EditText(this);
        durationField.setHint("Минут: 60");
        row.addView(durationField, new LinearLayout.LayoutParams(
                0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));

        locationField = new EditText(this);
        locationField.setHint("Место");
        row.addView(locationField, new LinearLayout.LayoutParams(
                0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        root.addView(row);

        LinearLayout actions = new LinearLayout(this);
        actions.setOrientation(LinearLayout.HORIZONTAL);

        Button create = new Button(this);
        create.setText("Создать");
        create.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                create();
            }
        });
        actions.addView(create);

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

    /** Создать событие из формы. */
    private void create() {
        final String title = titleField.getText().toString().trim();
        final String startText = startField.getText().toString().trim();
        final String durationText = durationField.getText().toString().trim();
        final String location = locationField.getText().toString().trim();

        if (title.isEmpty()) {
            status.setText("Нужно название.");
            return;
        }
        long startsAt;
        try {
            SimpleDateFormat format = new SimpleDateFormat(INPUT_FORMAT, Locale.getDefault());
            Date date = format.parse(startText);
            if (date == null) {
                throw new IllegalArgumentException("пустая дата");
            }
            startsAt = date.getTime() / 1000L;
        } catch (Exception error) {
            status.setText("Начало в формате " + INPUT_FORMAT + " (например, 2026-10-18 15:00).");
            return;
        }
        long minutes = 60;
        if (!durationText.isEmpty()) {
            try {
                minutes = Math.max(1L, Long.parseLong(durationText));
            } catch (NumberFormatException error) {
                status.setText("Длительность — число минут.");
                return;
            }
        }
        final long endsAt = startsAt + minutes * 60L;

        status.setText("Создаю…");
        final String server = Prefs.server(this);
        final String bearer = Prefs.coreToken(this);

        Background.run(new Background.Task<JSONObject>() {
            @Override
            public JSONObject run() throws Exception {
                return CoreClient.createEvent(server, bearer, title, startsAt, endsAt, location);
            }
        }, new Background.Callback<JSONObject>() {
            @Override
            public void onResult(JSONObject event) {
                status.setText("Создано: " + event.optString("title"));
                titleField.setText("");
                startField.setText("");
                durationField.setText("");
                locationField.setText("");
                load();
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка: " + error.getMessage());
            }
        });
    }

    /** Загрузить события. */
    private void load() {
        final String server = Prefs.server(this);
        final String bearer = Prefs.coreToken(this);
        Background.run(new Background.Task<JSONArray>() {
            @Override
            public JSONArray run() throws Exception {
                return CoreClient.calendarEvents(server, bearer);
            }
        }, new Background.Callback<JSONArray>() {
            @Override
            public void onResult(JSONArray events) {
                list.removeAllViews();
                if (events.length() == 0) {
                    status.setText("Событий нет.");
                    return;
                }
                status.setText("");
                SimpleDateFormat time = new SimpleDateFormat("dd.MM HH:mm", Locale.getDefault());
                for (int i = 0; i < events.length(); i++) {
                    JSONObject event = events.optJSONObject(i);
                    if (event == null) {
                        continue;
                    }
                    long startsAt = event.optLong("starts_at", 0);
                    long endsAt = event.optLong("ends_at", 0);
                    String location = event.optString("location", "");
                    TextView item = new TextView(CalendarActivity.this);
                    item.setText(time.format(new Date(startsAt * 1000L))
                            + "–" + time.format(new Date(endsAt * 1000L))
                            + " · " + event.optString("title", "?")
                            + (location.isEmpty() ? "" : " (" + location + ")"));
                    item.setTextSize(14);
                    item.setPadding(0, 10, 0, 10);
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
