package com.tensorlogix.tensa;

import android.app.PendingIntent;
import android.appwidget.AppWidgetManager;
import android.appwidget.AppWidgetProvider;
import android.content.Context;
import android.content.Intent;
import android.widget.RemoteViews;

/**
 * Домашний виджет Tensa: компактная зелёная пилюля на рабочем столе,
 * как desktop-виджет. Клик по микрофону/центральной кнопке открывает чат,
 * по «•••» — настройки.
 */
public class TensaWidgetProvider extends AppWidgetProvider {

    @Override
    public void onUpdate(Context context, AppWidgetManager manager, int[] ids) {
        for (int id : ids) {
            RemoteViews views = build(context);
            manager.updateAppWidget(id, views);
        }
    }

    /** Собрать RemoteViews с кликами. */
    private RemoteViews build(Context context) {
        String pkg = context.getPackageName();
        int layout = context.getResources().getIdentifier("tensa_widget", "layout", pkg);
        RemoteViews views = new RemoteViews(pkg, layout);

        int mic = context.getResources().getIdentifier("widget_mic", "id", pkg);
        int letter = context.getResources().getIdentifier("widget_letter", "id", pkg);
        int dots = context.getResources().getIdentifier("widget_dots", "id", pkg);

        Intent chat = new Intent(context, ChatActivity.class)
                .putExtra("from_widget", true);
        PendingIntent openChat = PendingIntent.getActivity(context, 11, chat,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        Intent voice = new Intent(context, ChatActivity.class)
                .putExtra("from_widget", true)
                .putExtra("widget_voice", true);
        PendingIntent openVoice = PendingIntent.getActivity(context, 12, voice,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        Intent settings = new Intent(context, SettingsActivity.class);
        PendingIntent openSettings = PendingIntent.getActivity(context, 13, settings,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);

        views.setOnClickPendingIntent(mic, openVoice);
        views.setOnClickPendingIntent(letter, openChat);
        views.setOnClickPendingIntent(dots, openSettings);
        return views;
    }
}
