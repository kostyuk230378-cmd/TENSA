package com.tensorlogix.tensa;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Intent;
import android.graphics.Color;
import android.graphics.PixelFormat;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.view.Gravity;
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
        if (web == null) {
            showOverlay();
        }
        return START_STICKY;
    }

    /** Показать плавающее окно с виджетом. */
    private void showOverlay() {
        windows = (WindowManager) getSystemService(WINDOW_SERVICE);
        web = new WebView(this);
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

        int width = dp(348);
        int height = dp(150);
        params = new WindowManager.LayoutParams(
                width,
                height,
                Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
                        ? WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
                        : WindowManager.LayoutParams.TYPE_PHONE,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                        | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                PixelFormat.TRANSLUCENT);
        params.gravity = Gravity.TOP | Gravity.END;
        params.x = dp(8);
        params.y = dp(64);

        web.loadUrl(server + "/widget");
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
                params.width = dp(440);
                params.height = dp(700);
                params.flags = WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS;
                windows.updateViewLayout(web, params);
                web.requestFocus();
                break;
            case "collapse":
                params.width = dp(348);
                params.height = dp(150);
                params.flags = WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                        | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS;
                windows.updateViewLayout(web, params);
                break;
            case "quit":
                stopSelf();
                break;
            default:
                break;
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
