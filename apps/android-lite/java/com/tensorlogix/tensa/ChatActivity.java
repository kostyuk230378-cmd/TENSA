package com.tensorlogix.tensa;

import android.os.Bundle;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import org.json.JSONObject;

/**
 * Чат: основной путь — ядро Tensa (`POST /api/v1/query`), оно само выбирает
 * облако OpenRouter (ключ в secrets.toml) или локальную GGUF.
 *
 * Если ядро ответило «не настроено» (503), а в настройках есть ключ
 * OpenRouter — резервный прямой вызов OpenRouter с телефона.
 */
public class ChatActivity extends android.app.Activity {

    /** Накопленная переписка. */
    private final StringBuilder transcript = new StringBuilder();
    /** Текст переписки. */
    private TextView conversation;
    /** Поле ввода. */
    private EditText input;
    /** Прокрутка переписки. */
    private ScrollView scroll;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(20, 20, 20, 12);

        TextView title = new TextView(this);
        title.setText("Чат · ядро Tensa");
        title.setTextSize(16);
        title.setPadding(0, 0, 0, 2);
        root.addView(title);

        TextView subtitle = new TextView(this);
        subtitle.setTextSize(12);
        subtitle.setText("По умолчанию — локальная модель ядра. Облако (OpenRouter) — "
                + "опция для больших моделей (26B).");
        subtitle.setPadding(0, 0, 0, 8);
        root.addView(subtitle);

        conversation = new TextView(this);
        conversation.setTextSize(16);
        conversation.setText("");
        scroll = new ScrollView(this);
        scroll.addView(conversation);
        LinearLayout.LayoutParams scrollParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f);
        root.addView(scroll, scrollParams);

        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);

        input = new EditText(this);
        input.setHint("Сообщение Tensa…");
        input.setMaxLines(3);
        input.setImeOptions(EditorInfo.IME_ACTION_SEND);
        LinearLayout.LayoutParams inputParams = new LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f);
        row.addView(input, inputParams);

        Button send = new Button(this);
        send.setText("→");
        send.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                send();
            }
        });
        row.addView(send);

        root.addView(row);
        setContentView(root);
    }

    /** Отправить текущее сообщение. */
    private void send() {
        final String text = input.getText().toString().trim();
        if (text.isEmpty()) {
            return;
        }
        input.setText("");
        appendLine("Вы: " + text);
        appendLine("Tensa: …");

        Background.run(new Background.Task<String>() {
            @Override
            public String run() throws Exception {
                return ask(text);
            }
        }, new Background.Callback<String>() {
            @Override
            public void onResult(String value) {
                replaceLastLine("Tensa: " + value);
            }

            @Override
            public void onError(Exception error) {
                replaceLastLine("Tensa: ошибка — " + error.getMessage());
            }
        });
    }

    /** Ядро, при 503/сетевой ошибке — прямой OpenRouter (ключ из настроек). */
    private String ask(String text) throws Exception {
        String server = Prefs.server(ChatActivity.this);
        String bearer = Prefs.coreToken(ChatActivity.this);
        if (!bearer.isEmpty()) {
            try {
                JSONObject response = CoreClient.query(server, bearer, text);
                String answer = response.optString("text", "");
                boolean degraded = response.optBoolean("degraded", false);
                String model = response.optString("model_used", "");
                return answer + (degraded ? "  ·  локальная модель" : "")
                        + (model.isEmpty() ? "" : "\n[" + model + "]");
            } catch (Exception coreError) {
                String directKey = Prefs.openRouterKey(ChatActivity.this);
                if (directKey.isEmpty()) {
                    throw new Exception("ядро: " + coreError.getMessage()
                            + " (прямой OpenRouter не настроен)");
                }
                // Резерв: прямой вызов облака с телефона.
            }
        } else {
            String directKey = Prefs.openRouterKey(ChatActivity.this);
            if (directKey.isEmpty()) {
                throw new Exception("не задан ни токен ядра, ни ключ OpenRouter — Настройки");
            }
        }
        return "напрямую OpenRouter:\n" + OpenRouterClient.chat(
                Prefs.openRouterKey(ChatActivity.this), Prefs.model(ChatActivity.this), text);
    }

    /** Добавить строку в переписку и прокрутить вниз. */
    private void appendLine(String line) {
        transcript.append(line).append('\n');
        conversation.setText(transcript.toString());
        scroll.post(new Runnable() {
            @Override
            public void run() {
                scroll.fullScroll(View.FOCUS_DOWN);
            }
        });
    }

    /** Заменить последнюю строку (заглушка «…» → ответ или ошибка). */
    private void replaceLastLine(String line) {
        int cut = transcript.lastIndexOf("\nTensa: ");
        if (cut >= 0) {
            transcript.setLength(cut + 1);
        }
        transcript.append(line).append('\n');
        conversation.setText(transcript.toString());
        scroll.post(new Runnable() {
            @Override
            public void run() {
                scroll.fullScroll(View.FOCUS_DOWN);
            }
        });
    }
}
