package com.tensorlogix.tensa;

import android.os.Bundle;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

/**
 * Чат с большой моделью через OpenRouter.
 *
 * Простой одноколоночный экран: история (в памяти), поле ввода, кнопка.
 * Ответ приходит в фоне, UI-поток не блокируется.
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
        title.setText("Чат · " + Prefs.model(this));
        title.setTextSize(16);
        title.setPadding(0, 0, 0, 8);
        root.addView(title);

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
                return OpenRouterClient.chat(Prefs.openRouterKey(ChatActivity.this),
                        Prefs.model(ChatActivity.this), text);
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
