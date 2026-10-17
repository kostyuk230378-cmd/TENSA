package com.tensorlogix.tensa;

import android.os.Bundle;
import android.text.InputType;
import android.view.View;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONObject;

import java.io.InputStream;

/**
 * Настройки: адрес ядра и его токен, ключ OpenRouter и модель,
 * установка модели эмбеддингов в ядро из assets APK.
 *
 * Кнопка «Установить модель» передаёт файлы e5-small в
 * `POST /api/v1/models/embedder/:file` — после этого `tensa memory`
 * работает на ПК без интернета.
 */
public class SettingsActivity extends android.app.Activity {

    /** Файлы модели в assets (порядок передачи). */
    private static final String[] MODEL_FILES = {
            "model_O4.onnx",
            "tokenizer.json",
            "config.json",
            "special_tokens_map.json",
            "tokenizer_config.json",
    };

    /** Кнопка установки модели. */
    private Button installButton;
    /** Статус установки модели. */
    private TextView modelStatus;

    /** Поле с подписью и вводом. */
    private EditText addField(LinearLayout root, String label, String value, int inputType) {
        TextView caption = new TextView(this);
        caption.setText(label);
        caption.setPadding(0, 18, 0, 4);
        root.addView(caption);

        EditText field = new EditText(this);
        field.setText(value);
        field.setInputType(inputType);
        root.addView(field);
        return field;
    }

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(32, 32, 32, 32);

        TextView title = new TextView(this);
        title.setText("Настройки");
        title.setTextSize(26);
        root.addView(title);

        final EditText server = addField(root, "Адрес ядра Tensa (ПК в Wi-Fi)",
                Prefs.server(this), InputType.TYPE_TEXT_VARIATION_URI);
        server.setHint("http://<ip-пк>:8765");
        final EditText coreToken = addField(root, "Токен ядра (файл pwa_token)",
                Prefs.coreToken(this), InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        final EditText openRouter = addField(root,
                "Ключ OpenRouter (опционально, большие модели 26B)",
                Prefs.openRouterKey(this), InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        final EditText model = addField(root, "Модель OpenRouter",
                Prefs.model(this), InputType.TYPE_CLASS_TEXT);

        Button save = new Button(this);
        save.setText("Сохранить");
        save.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                Prefs.save(SettingsActivity.this,
                        server.getText().toString(),
                        coreToken.getText().toString(),
                        openRouter.getText().toString(),
                        model.getText().toString());
                Toast.makeText(SettingsActivity.this, "Сохранено", Toast.LENGTH_SHORT).show();
                finish();
            }
        });
        root.addView(save);

        TextView modelCaption = new TextView(this);
        modelCaption.setText("Модель памяти (e5-small)");
        modelCaption.setTextSize(18);
        modelCaption.setPadding(0, 28, 0, 4);
        root.addView(modelCaption);

        modelStatus = new TextView(this);
        modelStatus.setText("Модель в APK: assets/multilingual-e5-small");
        modelStatus.setPadding(0, 4, 0, 8);
        root.addView(modelStatus);

        installButton = new Button(this);
        installButton.setText("Установить модель");
        installButton.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                installModel();
            }
        });
        root.addView(installButton);

        ScrollView scroll = new ScrollView(this);
        scroll.addView(root);
        setContentView(scroll);
    }

    /** Передать файлы модели из assets в ядро. */
    private void installModel() {
        installButton.setEnabled(false);
        modelStatus.setText("Проверяю модель у ядра…");
        final String server = Prefs.server(this);
        final String bearer = Prefs.coreToken(this);

        Background.run(new Background.Task<String>() {
            @Override
            public String run() throws Exception {
                JSONObject status = CoreClient.embedderStatus(server, bearer);
                if (status.optBoolean("installed", false)) {
                    return "Модель уже установлена у ядра.";
                }
                for (final String name : MODEL_FILES) {
                    runOnUiThread(new Runnable() {
                        @Override
                        public void run() {
                            modelStatus.setText("Передаю " + name + "…");
                        }
                    });
                    InputStream stream = getAssets().open("multilingual-e5-small/" + name);
                    try {
                        CoreClient.uploadEmbedderFile(server, bearer, name, stream);
                    } finally {
                        stream.close();
                    }
                }
                JSONObject after = CoreClient.embedderStatus(server, bearer);
                return after.optBoolean("installed", false)
                        ? "Модель установлена ✓ (ядро: " + after.optString("dir") + ")"
                        : "Передано не всё: " + after.optString("missing");
            }
        }, new Background.Callback<String>() {
            @Override
            public void onResult(String value) {
                installButton.setEnabled(true);
                modelStatus.setText(value);
                Toast.makeText(SettingsActivity.this, value, Toast.LENGTH_LONG).show();
            }

            @Override
            public void onError(Exception error) {
                installButton.setEnabled(true);
                modelStatus.setText("Ошибка: " + error.getMessage());
            }
        });
    }
}
