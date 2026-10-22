package com.tensorlogix.tensa;

import android.Manifest;
import android.app.DownloadManager;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.graphics.PixelFormat;
import android.media.AudioFormat;
import android.media.AudioRecord;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.util.DisplayMetrics;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.WindowManager;
import android.webkit.JavascriptInterface;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;

/**
 * Плавающий виджет Tensa поверх других приложений (SYSTEM_ALERT_WINDOW).
 *
 * Окно — зелёная пилюля из того же widget.html, что и на Windows; по IPC
 * «expand»/«collapse» сервис меняет размер overlay, «quit» — останавливает.
 * Требуется разрешение «Поверх других приложений» (выдаётся в Настройках).
 */
public class OverlayService extends Service {

    /** Канал уведомления foreground-сервиса. */
    private static final String CHANNEL = "tensa_overlay";
    /** id уведомления. */
    private static final int NOTIFICATION_ID = 41;

    /** Менеджер окон. */
    private WindowManager windows;
    /** Плавающий WebView. */
    private WebView web;
    /** Параметры окна. */
    private WindowManager.LayoutParams params;
    /** Адрес ядра. */
    private String server = "";
    /** Bearer-токен для API с телефона. */
    private String access = "";
    /** Ширина/высота экрана в пикселях. */
    private int screenWidth = 0;
    private int screenHeight = 0;
    /** Отступ overlay от края экрана. */
    private int margin = 0;
    /** Нативная запись для overlay-микрофона. */
    private static final int SAMPLE_RATE = 16000;
    /** Микрофон, выбранный в настройках Android. */
    private AudioRecord audioRecord;
    /** Развёрнута ли панель: тогда двигаем только за шапку, а содержимое скроллим. */
    private boolean expanded;
    private Thread audioThread;
    private ByteArrayOutputStream audioBytes;
    private volatile boolean recording;
    /** UI-поток для команд из JS. */
    private final Handler handler = new Handler(Looper.getMainLooper());

    @Override
    public void onCreate() {
        super.onCreate();
        createChannel();
        startForeground(NOTIFICATION_ID, notification());
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String extra = intent == null ? null : intent.getStringExtra("server");
        server = extra == null || extra.trim().isEmpty() ? Prefs.server(this) : extra.trim();
        String tokenExtra = intent == null ? null : intent.getStringExtra("token");
        access = tokenExtra == null || tokenExtra.trim().isEmpty()
                ? Prefs.coreToken(this) : tokenExtra.trim();
        if (web == null) {
            showOverlay();
        }
        return START_STICKY;
    }

    /** Показать плавающее окно с виджетом. */
    private void showOverlay() {
        windows = (WindowManager) getSystemService(WINDOW_SERVICE);
        DisplayMetrics metrics = getResources().getDisplayMetrics();
        screenWidth = metrics.widthPixels;
        screenHeight = metrics.heightPixels;
        margin = dp(8);
        web = new DragWebView(this);
        web.setBackgroundColor(Color.TRANSPARENT);
        WebSettings settings = web.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        web.setVerticalScrollBarEnabled(false);
        web.setHorizontalScrollBarEnabled(false);
        web.addJavascriptInterface(new Bridge(), "TensaHost");
        web.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageFinished(WebView view, String url) {
                // Мост как у wry: window.ipc.postMessage(...)
                view.evaluateJavascript(
                        "window.ipc = { postMessage: function(m){ TensaHost.postMessage(String(m)); } };",
                        null);
            }
        });

        int width = Math.min(dp(348), screenWidth - margin * 2);
        int height = Math.min(dp(150), screenHeight - dp(64) - margin);
        params = new WindowManager.LayoutParams(
                width,
                height,
                Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
                        ? WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
                        : WindowManager.LayoutParams.TYPE_PHONE,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE,
                PixelFormat.TRANSLUCENT);
        params.gravity = Gravity.TOP | Gravity.START;
        params.x = screenWidth - width - margin;
        params.y = dp(64);

        web.loadUrl(server + "/widget?overlay=1&token=" + Uri.encode(access));
        windows.addView(web, params);
    }

    /** Команды из виджета. */
    private final class Bridge {
        /** Принять сообщение из JS. */
        @JavascriptInterface
        public void postMessage(final String message) {
            handler.post(new Runnable() {
                @Override
                public void run() {
                    handle(message);
                }
            });
        }

        /** Нативный push-to-talk: старт записи. */
        @JavascriptInterface
        public void startVoice() {
            handler.post(new Runnable() {
                @Override
                public void run() {
                    startOverlayVoice();
                }
            });
        }

        /** Нативный push-to-talk: стоп и ASR на ПК. */
        @JavascriptInterface
        public void stopVoice() {
            handler.post(new Runnable() {
                @Override
                public void run() {
                    stopOverlayVoice();
                }
            });
        }

        /** Все входы Android для списка микрофонов в настройках виджета. */
        @JavascriptInterface
        public String listMics() {
            JSONArray result = new JSONArray();
            for (AudioInputs.Device device : AudioInputs.devices(OverlayService.this)) {
                try {
                    JSONObject item = new JSONObject();
                    item.put("id", device.id);
                    item.put("name", device.name);
                    item.put("type", AudioInputs.typeName(device.type));
                    result.put(item);
                } catch (Exception ignored) {
                    // одно устройство не должно ломать весь список
                }
            }
            return result.toString();
        }

        /** Id сохранённого микрофона (0 — системный). */
        @JavascriptInterface
        public int selectedMicId() {
            return Prefs.micId(OverlayService.this);
        }

        /** Сохранить выбранный микрофон; применяется к следующей записи. */
        @JavascriptInterface
        public void selectMic(int id, String name) {
            Prefs.setMic(OverlayService.this, id, name == null ? "" : name);
        }

        /** Открыть полный дашборд Tensa из виджета (кнопка «Все разделы»). */
        @JavascriptInterface
        public void openApp() {
            handler.post(new Runnable() {
                @Override
                public void run() {
                    Intent intent = new Intent(OverlayService.this, MainActivity.class);
                    intent.putExtra("full", true);
                    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                    startActivity(intent);
                }
            });
        }

        /** Скачать найденный файл с ПК в «Загрузки» телефона. */
        @JavascriptInterface
        public void downloadFile(String path, String name) {
            if (path == null || path.trim().isEmpty()) {
                return;
            }
            String fileName = (name == null || name.trim().isEmpty()) ? "tensa-file" : name.trim();
            fileName = fileName.replace('/', '_').replace('\\', '_');
            try {
                DownloadManager manager = (DownloadManager) getSystemService(DOWNLOAD_SERVICE);
                if (manager == null) {
                    return;
                }
                Uri uri = Uri.parse(server + "/api/v1/files/download?path=" + Uri.encode(path));
                DownloadManager.Request request = new DownloadManager.Request(uri);
                request.addRequestHeader("Authorization", "Bearer " + access);
                request.setTitle("Tensa: " + fileName);
                request.setDescription("Файл с ПК");
                request.setNotificationVisibility(
                        DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED);
                request.setDestinationInExternalPublicDir(
                        Environment.DIRECTORY_DOWNLOADS, "Tensa/" + fileName);
                manager.enqueue(request);
            } catch (Exception ignored) {
                // DownloadManager сам покажет ошибку в уведомлении
            }
        }
    }

    /** Нативный старт записи для overlay-микрофона. */
    private void startOverlayVoice() {
        if (recording) {
            return;
        }
        if (server.isEmpty() || access.isEmpty()) {
            evalVoiceError("Подключите ПК в Настройках.");
            return;
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO)
                != PackageManager.PERMISSION_GRANTED) {
            evalVoiceError("Разрешите микрофон в приложении.");
            return;
        }
        try {
            int min = AudioRecord.getMinBufferSize(SAMPLE_RATE,
                    AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT);
            if (min <= 0) {
                throw new IllegalStateException("микрофон не поддерживает 16 кГц");
            }
            audioRecord = new AudioRecord(AudioInputs.source(this), SAMPLE_RATE,
                    AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT,
                    Math.max(min, SAMPLE_RATE));
            if (audioRecord.getState() != AudioRecord.STATE_INITIALIZED) {
                throw new IllegalStateException("не удалось открыть микрофон");
            }
            AudioInputs.apply(audioRecord, this);
            audioBytes = new ByteArrayOutputStream();
            recording = true;
            audioRecord.startRecording();
            audioThread = new Thread(new Runnable() {
                @Override
                public void run() {
                    byte[] buffer = new byte[3200];
                    while (recording) {
                        int read = audioRecord.read(buffer, 0, buffer.length);
                        if (read > 0) {
                            audioBytes.write(buffer, 0, read);
                        } else if (read < 0) {
                            break;
                        }
                    }
                }
            }, "tensa-overlay-mic");
            audioThread.start();
        } catch (Exception error) {
            stopOverlayVoice();
            evalVoiceError("Микрофон: " + error.getMessage());
        }
    }

    /** Остановить overlay-запись и отправить WAV в ASR на ПК. */
    private void stopOverlayVoice() {
        if (!recording) {
            return;
        }
        recording = false;
        try {
            if (audioRecord != null) {
                audioRecord.stop();
            }
        } catch (Exception ignored) {
            // уже остановлен
        }
        try {
            if (audioThread != null) {
                audioThread.join(1500);
            }
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
        if (audioRecord != null) {
            audioRecord.release();
            audioRecord = null;
        }
        audioThread = null;
        AudioInputs.release(this);
        byte[] pcm = audioBytes == null ? new byte[0] : audioBytes.toByteArray();
        audioBytes = null;
        if (pcm.length < SAMPLE_RATE / 5) {
            evalVoiceError("Слишком короткая запись — удерживайте микрофон.");
            return;
        }
        final byte[] wav = wavFromPcm(pcm);
        new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    String text = CoreClient.transcribeAudio(server, access, wav);
                    final String js = "window.tensaVoiceResult(" + JSONObject.quote(text) + ");";
                    handler.post(new Runnable() {
                        @Override
                        public void run() {
                            if (web != null) {
                                web.evaluateJavascript(js, null);
                            }
                        }
                    });
                } catch (final Exception error) {
                    evalVoiceError(error.getMessage() == null ? "ASR недоступен" : error.getMessage());
                }
            }
        }, "tensa-overlay-asr").start();
    }

    private void evalVoiceError(final String message) {
        handler.post(new Runnable() {
            @Override
            public void run() {
                if (web != null) {
                    web.evaluateJavascript("window.tensaVoiceError(" + JSONObject.quote(message) + ");", null);
                }
            }
        });
    }

    private byte[] wavFromPcm(byte[] pcm) {
        ByteBuffer buffer = ByteBuffer.allocate(44 + pcm.length).order(ByteOrder.LITTLE_ENDIAN);
        buffer.put("RIFF".getBytes(StandardCharsets.US_ASCII));
        buffer.putInt(36 + pcm.length);
        buffer.put("WAVE".getBytes(StandardCharsets.US_ASCII));
        buffer.put("fmt ".getBytes(StandardCharsets.US_ASCII));
        buffer.putInt(16);
        buffer.putShort((short) 1);
        buffer.putShort((short) 1);
        buffer.putInt(SAMPLE_RATE);
        buffer.putInt(SAMPLE_RATE * 2);
        buffer.putShort((short) 2);
        buffer.putShort((short) 16);
        buffer.put("data".getBytes(StandardCharsets.US_ASCII));
        buffer.putInt(pcm.length);
        buffer.put(pcm);
        return buffer.array();
    }

    /** Обработать команду виджета. */
    private void handle(String message) {
        if (web == null || params == null) {
            return;
        }
        switch (message) {
            case "expand":
                expanded = true;
                params.width = Math.min(dp(440), screenWidth - margin * 2);
                params.height = Math.min(dp(700), screenHeight - dp(64) - margin);
                params.x = margin;
                params.y = dp(64);
                params.flags = 0;
                windows.updateViewLayout(web, params);
                web.requestFocus();
                break;
            case "collapse":
                expanded = false;
                params.width = Math.min(dp(348), screenWidth - margin * 2);
                params.height = Math.min(dp(150), screenHeight - dp(64) - margin);
                params.x = screenWidth - params.width - margin;
                params.y = dp(64);
                params.flags = WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE;
                windows.updateViewLayout(web, params);
                break;
            case "quit":
                Prefs.setOverlayEnabled(this, false);
                stopSelf();
                break;
            case "app":
                Intent full = new Intent(this, MainActivity.class);
                full.putExtra("full", true);
                full.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                startActivity(full);
                break;
            default:
                if (message.startsWith("move ")) {
                    moveBy(message);
                }
                break;
        }
    }

    /** Переместить overlay по пиксельным дельтам из widget.html. */
    private void moveBy(String message) {
        String[] parts = message.split(" ");
        if (parts.length < 3) {
            return;
        }
        try {
            int dx = Integer.parseInt(parts[1].trim());
            int dy = Integer.parseInt(parts[2].trim());
            moveWindow(dx, dy);
        } catch (NumberFormatException ignored) {
            // повреждённая команда движения — игнорируем
        }
    }

    /** Сдвинуть окно с ограничением по границам экрана. */
    private void moveWindow(int dx, int dy) {
        if (params == null) {
            return;
        }
        params.x = Math.max(0, Math.min(screenWidth - params.width, params.x + dx));
        params.y = Math.max(0, Math.min(screenHeight - params.height, params.y + dy));
        if (windows != null && web != null) {
            windows.updateViewLayout(web, params);
        }
    }

    /** WebView, который двигает overlay за любое место, а не только за dock. */
    private final class DragWebView extends WebView {
        private float downX;
        private float downY;
        private float lastX;
        private float lastY;
        private boolean dragging;
        private boolean dragAllowed;

        DragWebView(android.content.Context context) {
            super(context);
        }

        @Override
        public boolean onTouchEvent(MotionEvent event) {
            float rawX = event.getRawX();
            float rawY = event.getRawY();
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    downX = rawX;
                    downY = rawY;
                    lastX = rawX;
                    lastY = rawY;
                    dragging = false;
                    dragAllowed = !expanded || event.getY() <= dp(58);
                    return super.onTouchEvent(event);
                case MotionEvent.ACTION_MOVE:
                    if (!dragAllowed) {
                        return super.onTouchEvent(event);
                    }
                    if (!dragging && (Math.abs(rawX - downX) + Math.abs(rawY - downY)) > dp(10)) {
                        dragging = true;
                    }
                    if (dragging) {
                        int dx = Math.round(rawX - lastX);
                        int dy = Math.round(rawY - lastY);
                        lastX = rawX;
                        lastY = rawY;
                        moveWindow(dx, dy);
                        return true;
                    }
                    return super.onTouchEvent(event);
                case MotionEvent.ACTION_UP:
                case MotionEvent.ACTION_CANCEL:
                    if (dragging) {
                        dragging = false;
                        return true;
                    }
                    return super.onTouchEvent(event);
                default:
                    return super.onTouchEvent(event);
            }
        }
    }

    /** Плотность экрана в пикселях. */
    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    /** Канал уведомлений. */
    private void createChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            NotificationChannel channel = new NotificationChannel(
                    CHANNEL,
                    "Tensa widget",
                    NotificationManager.IMPORTANCE_LOW);
            NotificationManager manager = getSystemService(NotificationManager.class);
            if (manager != null) {
                manager.createNotificationChannel(channel);
            }
        }
    }

    /** Уведомление foreground-сервиса. */
    private Notification notification() {
        Notification.Builder builder = Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
                ? new Notification.Builder(this, CHANNEL)
                : new Notification.Builder(this);
        return builder
                .setContentTitle("Tensa")
                .setContentText("Виджет поверх приложений")
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setOngoing(true)
                .build();
    }

    @Override
    public void onDestroy() {
        recording = false;
        if (audioRecord != null) {
            try {
                audioRecord.stop();
            } catch (Exception ignored) {
                // уже остановлен
            }
            audioRecord.release();
            audioRecord = null;
        }
        if (windows != null && web != null) {
            windows.removeView(web);
            web.destroy();
            web = null;
        }
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
