package com.tensorlogix.tensa;

import android.Manifest;
import android.app.DownloadManager;
import android.content.Context;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONArray;
import org.json.JSONObject;

import java.net.URLEncoder;

/**
 * Файлы на ПК: поиск по индексу ядра и скачивание выбранного документа
 * в Downloads телефона.
 */
public class FilesActivity extends android.app.Activity {

    /** Поле запроса. */
    private EditText query;
    /** Статус. */
    private TextView status;
    /** Результаты. */
    private LinearLayout results;
    /** Отложенное скачивание до выдачи разрешения на старых Android. */
    private String pendingServer;
    private String pendingBearer;
    private String pendingPath;
    private String pendingName;

    /** Код разрешения хранилища (Android 8–9). */
    private static final int REQUEST_STORAGE = 42;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Ui.style(this);

        LinearLayout root = Ui.column(this);
        root.setPadding(Ui.dp(this, 20), Ui.dp(this, 34),
                Ui.dp(this, 20), Ui.dp(this, 16));
        root.addView(Ui.header(this, "Файлы", true));

        LinearLayout searchRow = Ui.row(this);
        query = Ui.field(this, "Договор, отчёт, смета…", "",
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        query.setSingleLine(true);
        searchRow.addView(query, new LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        TextView find = Ui.button(this, "Найти", Ui.GREEN, Ui.BG);
        LinearLayout.LayoutParams findParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        findParams.leftMargin = Ui.dp(this, 8);
        searchRow.addView(find, findParams);
        find.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                search();
            }
        });
        root.addView(searchRow);

        status = Ui.muted(this, "Поиск идёт по именам и тексту документов на ПК.");
        status.setPadding(0, Ui.dp(this, 10), 0, Ui.dp(this, 10));
        root.addView(status);

        results = Ui.column(this);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(results);
        root.addView(scroll, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));

        setContentView(root);
    }

    /** Запросить индекс ядра. */
    private void search() {
        final String text = query.getText().toString().trim();
        if (text.isEmpty()) {
            status.setText("Введите запрос.");
            return;
        }
        final String server = Prefs.server(this);
        final String bearer = Prefs.coreToken(this);
        if (server.isEmpty() || bearer.isEmpty()) {
            status.setText("Сначала подключите ПК в Настройках.");
            return;
        }
        status.setText("Ищу на ПК…");
        results.removeAllViews();
        Background.run(new Background.Task<JSONArray>() {
            @Override
            public JSONArray run() throws Exception {
                return CoreClient.searchFiles(server, bearer, text, 20);
            }
        }, new Background.Callback<JSONArray>() {
            @Override
            public void onResult(JSONArray files) {
                status.setText(files.length() == 0 ? "Ничего не найдено." : "Найдено: " + files.length());
                for (int index = 0; index < files.length(); index++) {
                    JSONObject file = files.optJSONObject(index);
                    if (file == null) {
                        continue;
                    }
                    addFile(file, server, bearer);
                }
            }

            @Override
            public void onError(Exception error) {
                status.setText("Ошибка: " + error.getMessage());
            }
        });
    }

    /** Карточка файла с кнопкой скачивания. */
    private void addFile(final JSONObject file, final String server, final String bearer) {
        final String path = file.optString("path", "");
        final String name = file.optString("name", "файл");
        String snippet = file.optString("snippet", "");
        if (snippet.length() > 220) {
            snippet = snippet.substring(0, 220) + "…";
        }
        LinearLayout card = Ui.card(this);
        TextView title = Ui.text(this, name, 16, Ui.TEXT, android.graphics.Typeface.BOLD);
        card.addView(title);
        if (!snippet.isEmpty()) {
            TextView body = Ui.muted(this, snippet);
            body.setTextSize(12);
            body.setPadding(0, Ui.dp(this, 6), 0, 0);
            card.addView(body);
        }
        TextView pathView = Ui.text(this, path, 10, Ui.MUTED, android.graphics.Typeface.NORMAL);
        pathView.setTextSize(10);
        pathView.setPadding(0, Ui.dp(this, 6), 0, 0);
        card.addView(pathView);
        TextView download = Ui.secondary(this, "Скачать на телефон");
        LinearLayout.LayoutParams downloadParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        downloadParams.topMargin = Ui.dp(this, 10);
        downloadParams.gravity = Gravity.END;
        card.addView(download, downloadParams);
        download.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                download(server, bearer, path, name);
            }
        });
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        params.bottomMargin = Ui.dp(this, 10);
        results.addView(card, params);
    }

    /** Поставить файл в DownloadManager с Bearer-токеном. */
    private void download(String server, String bearer, String path, String name) {
        if (Build.VERSION.SDK_INT < 29
                && checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE)
                != PackageManager.PERMISSION_GRANTED) {
            pendingServer = server;
            pendingBearer = bearer;
            pendingPath = path;
            pendingName = name;
            requestPermissions(new String[]{Manifest.permission.WRITE_EXTERNAL_STORAGE},
                    REQUEST_STORAGE);
            return;
        }
        try {
            String encoded = URLEncoder.encode(path, "UTF-8");
            DownloadManager.Request request = new DownloadManager.Request(
                    Uri.parse(server + "/api/v1/files/download?path=" + encoded));
            request.addRequestHeader("Authorization", "Bearer " + bearer);
            request.setTitle(name);
            request.setDescription("Tensa · скачивание с ПК");
            request.setNotificationVisibility(
                    DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED);
            request.setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name);
            DownloadManager manager = (DownloadManager) getSystemService(Context.DOWNLOAD_SERVICE);
            if (manager == null) {
                throw new IllegalStateException("DownloadManager недоступен");
            }
            manager.enqueue(request);
            Toast.makeText(this, "Скачиваю в Downloads…", Toast.LENGTH_SHORT).show();
        } catch (Exception error) {
            Toast.makeText(this, "Не удалось скачать: " + error.getMessage(),
                    Toast.LENGTH_LONG).show();
        }
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(requestCode, permissions, results);
        if (requestCode == REQUEST_STORAGE && results.length > 0
                && results[0] == PackageManager.PERMISSION_GRANTED
                && pendingPath != null) {
            download(pendingServer, pendingBearer, pendingPath, pendingName);
        }
    }
}
