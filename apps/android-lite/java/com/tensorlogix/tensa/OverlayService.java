package com.tensorlogix.tensa;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Intent;
import android.graphics.Color;
import android.graphics.PixelFormat;
import android.net.Uri;
import android.os.Build;
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
    }

    /** Обработать команду виджета. */
    private void handle(String message) {
        if (web == null || params == null) {
            return;
        }
        switch (message) {
            case "expand":
                params.width = Math.min(dp(440), screenWidth - margin * 2);
                params.height = Math.min(dp(700), screenHeight - dp(64) - margin);
                params.x = margin;
                params.y = dp(64);
                params.flags = 0;
                windows.updateViewLayout(web, params);
                web.requestFocus();
                break;
            case "collapse":
                params.width = Math.min(dp(348), screenWidth - margin * 2);
                params.height = Math.min(dp(150), screenHeight - dp(64) - margin);
                params.x = screenWidth - params.width - margin;
                params.y = dp(64);
                params.flags = WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE;
                windows.updateViewLayout(web, params);
                break;
            case "quit":
                stopSelf();
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
                    return super.onTouchEvent(event);
                case MotionEvent.ACTION_MOVE:
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
