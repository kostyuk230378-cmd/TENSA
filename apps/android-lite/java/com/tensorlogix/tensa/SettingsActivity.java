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

/**
 * Настройки: адрес ядра и его токен, ключ OpenRouter и модель.
 *
 * Секреты хранятся в SharedPreferences приложения; в логи не попадают.
 */
public class SettingsActivity extends android.app.Activity {

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
        final EditText coreToken = addField(root, "Токен ядра (файл pwa_token)",
                Prefs.coreToken(this), InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        final EditText openRouter = addField(root, "Ключ OpenRouter (sk-or-…)",
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

        ScrollView scroll = new ScrollView(this);
        scroll.addView(root);
        setContentView(scroll);
    }
}
