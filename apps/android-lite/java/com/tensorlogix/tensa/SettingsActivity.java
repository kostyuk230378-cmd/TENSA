package com.tensorlogix.tensa;

import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.Settings;
import android.text.InputType;
import android.view.View;
import android.widget.AdapterView;
import android.widget.ArrayAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.Spinner;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONObject;

import java.io.InputStream;
import java.util.ArrayList;
import java.util.List;

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
    /** Список микрофонов телефона и выбранный вход. */
    private Spinner micSpinner;
    /** Статус микрофона. */
    private TextView micStatus;
    /** Текущие входы Android. */
    private List<AudioInputs.Device> micDevices = new ArrayList<>();
    /** Идёт обновление списка, не сохранять промежуточный выбор. */
    private boolean micUpdating;

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

        TextView where = new TextView(this);
        where.setText("Адрес показан на ПК в Настройках Tensa, над карточкой «Микрофон». "
                + "Токен лежит в %USERPROFILE%\\.tensa\\pwa_token.");
        where.setTextSize(11);
        where.setPadding(0, 6, 0, 4);
        root.addView(where);

        final TextView connectionStatus = new TextView(this);
        connectionStatus.setTextSize(12);
        connectionStatus.setPadding(0, 4, 0, 8);
        root.addView(connectionStatus);

        Button test = new Button(this);
        test.setText("Проверить подключение");
        test.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                final String serverValue = Prefs.normalize(server.getText().toString());
                final String tokenValue = coreToken.getText().toString().trim();
                if (serverValue.isEmpty() || tokenValue.isEmpty()) {
                    connectionStatus.setText("Введите адрес и токен.");
                    return;
                }
                connectionStatus.setText("Проверяю…");
                Background.run(new Background.Task<String>() {
                    @Override
                    public String run() throws Exception {
                        JSONObject health = CoreClient.health(serverValue, tokenValue);
                        return "Ядро на связи · Tensa " + health.optString("version", "");
                    }
                }, new Background.Callback<String>() {
                    @Override
                    public void onResult(String value) {
                        connectionStatus.setText(value);
                    }

                    @Override
                    public void onError(Exception error) {
                        connectionStatus.setText("Нет связи: " + error.getMessage());
                    }
                });
            }
        });
        root.addView(test);

        TextView micCaption = new TextView(this);
        micCaption.setText("Микрофон телефона и внешние");
        micCaption.setTextSize(18);
        micCaption.setPadding(0, 26, 0, 4);
        root.addView(micCaption);

        micStatus = new TextView(this);
        micStatus.setTextSize(12);
        micStatus.setLineSpacing(0, 1.2f);
        root.addView(micStatus);

        micSpinner = new Spinner(this);
        root.addView(micSpinner);

        Button micRefresh = new Button(this);
        micRefresh.setText("Обновить список микрофонов");
        micRefresh.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                refreshMicrophones();
                Toast.makeText(SettingsActivity.this,
                        "Подключите USB/Bluetooth-микрофон — он появится здесь.",
                        Toast.LENGTH_SHORT).show();
            }
        });
        root.addView(micRefresh);
        refreshMicrophones();

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
                Prefs.setOnboardingDone(SettingsActivity.this, true);
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

        TextView overlayCaption = new TextView(this);
        overlayCaption.setText("Плавающий виджет");
        overlayCaption.setTextSize(18);
        overlayCaption.setPadding(0, 28, 0, 4);
        root.addView(overlayCaption);

        final TextView overlayStatus = new TextView(this);
        overlayStatus.setText(Build.VERSION.SDK_INT >= Build.VERSION_CODES.M
                && Settings.canDrawOverlays(this)
                ? "Разрешение «поверх приложений» выдано."
                : "Нужно разрешение «Поверх других приложений».");
        overlayStatus.setPadding(0, 4, 0, 8);
        root.addView(overlayStatus);

        Button overlayStart = new Button(this);
        overlayStart.setText("Включить виджет");
        overlayStart.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M
                        && !Settings.canDrawOverlays(SettingsActivity.this)) {
                    Intent intent = new Intent(
                            Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                            Uri.parse("package:" + getPackageName()));
                    startActivity(intent);
                    return;
                }
                Intent service = new Intent(SettingsActivity.this, OverlayService.class)
                        .putExtra("server", Prefs.server(SettingsActivity.this))
                        .putExtra("token", Prefs.coreToken(SettingsActivity.this));
                Prefs.setOverlayEnabled(SettingsActivity.this, true);
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    startForegroundService(service);
                } else {
                    startService(service);
                }
                overlayStatus.setText("Виджет запущен — зелёная пилюля поверх приложений.");
            }
        });
        root.addView(overlayStart);

        Button overlayStop = new Button(this);
        overlayStop.setText("Остановить виджет");
        overlayStop.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                Prefs.setOverlayEnabled(SettingsActivity.this, false);
                stopService(new Intent(SettingsActivity.this, OverlayService.class));
                overlayStatus.setText("Виджет остановлен.");
            }
        });
        root.addView(overlayStop);

        ScrollView scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        scroll.setVerticalScrollBarEnabled(true);
        scroll.addView(root);
        setContentView(scroll);
    }


    /** Показать все входы Android: встроенный, USB, Bluetooth, гарнитуру. */
    private void refreshMicrophones() {
        if (micSpinner == null || micStatus == null) {
            return;
        }
        micUpdating = true;
        micDevices = AudioInputs.devices(this);
        List<String> labels = new ArrayList<>();
        labels.add("Системный микрофон (авто)");
        for (AudioInputs.Device device : micDevices) {
            labels.add(device.label());
        }
        ArrayAdapter<String> adapter = new ArrayAdapter<>(this,
                android.R.layout.simple_spinner_item, labels);
        adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item);
        micSpinner.setAdapter(adapter);

        int saved = Prefs.micId(this);
        int position = 0;
        for (int i = 0; i < micDevices.size(); i++) {
            if (micDevices.get(i).id == saved) {
                position = i + 1;
                break;
            }
        }
        micSpinner.setSelection(position);
        final int selectedPosition = position;
        micSpinner.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
            @Override
            public void onItemSelected(AdapterView<?> parent, View view, int index, long id) {
                if (micUpdating) {
                    return;
                }
                if (index == selectedPosition) {
                    return;
                }
                if (index <= 0 || index > micDevices.size()) {
                    Prefs.setMic(SettingsActivity.this, 0, "");
                    micStatus.setText("Системный микрофон: Android сам выберет лучший вход.");
                    return;
                }
                AudioInputs.Device device = micDevices.get(index - 1);
                Prefs.setMic(SettingsActivity.this, device.id, device.name);
                micStatus.setText("Выбран: " + device.name
                        + ". Запись в чате и виджете пойдёт с него.");
                Toast.makeText(SettingsActivity.this, "Микрофон: " + device.name,
                        Toast.LENGTH_SHORT).show();
            }

            @Override
            public void onNothingSelected(AdapterView<?> parent) {
                // оставляем текущий выбор
            }
        });
        micUpdating = false;

        if (micDevices.isEmpty()) {
            micStatus.setText("Система не отдала список микрофонов. Проверьте разрешение на запись.");
        } else if (saved != 0 && position == 0) {
            micStatus.setText("Выбранный ранее микрофон сейчас не подключён. "
                    + "Запись идёт с системного; подключите его и нажмите «Обновить».");
        } else if (saved == 0) {
            micStatus.setText("Найдено входов: " + micDevices.size()
                    + ". Сейчас выбран системный микрофон.");
        } else {
            micStatus.setText("Выбран: " + Prefs.micName(this)
                    + ". Запись в чате и виджете идёт с него.");
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (micSpinner != null) {
            refreshMicrophones();
        }
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
