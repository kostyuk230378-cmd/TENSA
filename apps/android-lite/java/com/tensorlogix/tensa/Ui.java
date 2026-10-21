package com.tensorlogix.tensa;

import android.app.Activity;
import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.Window;
import android.widget.EditText;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * Единый визуальный язык мобильной Tensa: тёмный HUD, зелёный акцент,
 * скруглённые карточки и крупные касаемые элементы.
 */
public final class Ui {

    /** Основной фон. */
    public static final int BG = Color.rgb(9, 9, 11);
    /** Поверхность карточки. */
    public static final int CARD = Color.rgb(18, 18, 21);
    /** Граница. */
    public static final int BORDER = Color.rgb(39, 39, 42);
    /** Циан. */
    public static final int CYAN = Color.rgb(0, 240, 255);
    /** Зелёный. */
    public static final int GREEN = Color.rgb(0, 255, 0);
    /** Приглушённый текст. */
    public static final int MUTED = Color.rgb(161, 161, 170);
    /** Красный. */
    public static final int RED = Color.rgb(255, 0, 85);
    /** Основной текст. */
    public static final int TEXT = Color.WHITE;

    private Ui() {
    }

    /** dp → px. */
    public static int dp(Context context, int value) {
        return Math.round(value * context.getResources().getDisplayMetrics().density);
    }

    /** Тёмный системный фон активности. */
    public static void style(Activity activity) {
        Window window = activity.getWindow();
        window.setStatusBarColor(BG);
        window.setNavigationBarColor(BG);
        View decor = window.getDecorView();
        decor.setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
        activity.getWindow().getDecorView().setBackgroundColor(BG);
    }

    /** Колонка без отступов. */
    public static LinearLayout column(Context context) {
        LinearLayout layout = new LinearLayout(context);
        layout.setOrientation(LinearLayout.VERTICAL);
        return layout;
    }

    /** Горизонтальный ряд. */
    public static LinearLayout row(Context context) {
        LinearLayout layout = new LinearLayout(context);
        layout.setOrientation(LinearLayout.HORIZONTAL);
        layout.setGravity(Gravity.CENTER_VERTICAL);
        return layout;
    }

    /** Заголовок экрана. */
    public static TextView title(Context context, String text) {
        TextView view = text(context, text, 26, TEXT, Typeface.BOLD);
        view.setPadding(0, 0, 0, dp(context, 6));
        return view;
    }

    /** Подпись/описание. */
    public static TextView muted(Context context, String text) {
        return text(context, text, 13, MUTED, Typeface.NORMAL);
    }

    /** Текстовое поле. */
    public static TextView text(Context context, String value, float size, int color, int style) {
        TextView view = new TextView(context);
        view.setText(value);
        view.setTextSize(size);
        view.setTextColor(color);
        view.setTypeface(Typeface.create("sans-serif", style));
        return view;
    }

    /** Скруглённый фон. */
    public static GradientDrawable box(int color, int stroke, float radius) {
        GradientDrawable drawable = new GradientDrawable();
        drawable.setColor(color);
        drawable.setCornerRadius(radius);
        if (stroke != 0) {
            drawable.setStroke(1, stroke);
        }
        return drawable;
    }

    /** Карточка с отступами. */
    public static LinearLayout card(Context context) {
        LinearLayout card = column(context);
        card.setPadding(dp(context, 16), dp(context, 14), dp(context, 16), dp(context, 14));
        card.setBackground(box(CARD, BORDER, dp(context, 16)));
        return card;
    }

    /** Крупная кнопка действия. */
    public static TextView button(Context context, String label, int background, int foreground) {
        TextView view = new TextView(context);
        view.setText(label);
        view.setTextSize(15);
        view.setTextColor(foreground);
        view.setTypeface(Typeface.DEFAULT_BOLD);
        view.setGravity(Gravity.CENTER);
        view.setPadding(dp(context, 16), dp(context, 13), dp(context, 16), dp(context, 13));
        view.setBackground(box(background, 0, dp(context, 12)));
        view.setClickable(true);
        view.setFocusable(true);
        view.setBackgroundTintList(ColorStateList.valueOf(background));
        return view;
    }

    /** Вторичная кнопка. */
    public static TextView secondary(Context context, String label) {
        TextView view = button(context, label, CARD, CYAN);
        view.setBackground(box(CARD, BORDER, dp(context, 12)));
        return view;
    }

    /** Поле ввода. */
    public static EditText field(Context context, String hint, String value, int inputType) {
        EditText field = new EditText(context);
        field.setText(value == null ? "" : value);
        field.setHint(hint);
        field.setHintTextColor(MUTED);
        field.setTextColor(TEXT);
        field.setTextSize(15);
        field.setInputType(inputType);
        field.setSingleLine(false);
        field.setPadding(dp(context, 14), dp(context, 12), dp(context, 14), dp(context, 12));
        field.setBackground(box(Color.rgb(12, 12, 15), BORDER, dp(context, 12)));
        return field;
    }

    /** Логотип приложения. */
    public static ImageView logo(Context context, int sizeDp) {
        ImageView image = new ImageView(context);
        int id = context.getResources().getIdentifier("ic_launcher", "mipmap", context.getPackageName());
        if (id != 0) {
            image.setImageResource(id);
        }
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                dp(context, sizeDp), dp(context, sizeDp));
        image.setLayoutParams(params);
        image.setScaleType(ImageView.ScaleType.FIT_CENTER);
        return image;
    }

    /** Верхняя панель с назад/заголовком. */
    public static LinearLayout header(Context context, String titleText, boolean back) {
        LinearLayout header = row(context);
        header.setPadding(0, 0, 0, dp(context, 14));
        if (back) {
            TextView button = text(context, "‹", 32, CYAN, Typeface.NORMAL);
            button.setGravity(Gravity.CENTER);
            button.setPadding(dp(context, 4), 0, dp(context, 16), 0);
            button.setOnClickListener(new View.OnClickListener() {
                @Override
                public void onClick(View view) {
                    ((Activity) context).finish();
                }
            });
            header.addView(button);
        }
        TextView titleView = text(context, titleText, 22, TEXT, Typeface.BOLD);
        titleView.setLayoutParams(new LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        header.addView(titleView);
        return header;
    }

    /** Статусная точка. */
    public static View dot(Context context, int color) {
        View dot = new View(context);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                dp(context, 8), dp(context, 8));
        params.setMargins(0, 0, dp(context, 8), 0);
        dot.setLayoutParams(params);
        dot.setBackground(box(color, 0, dp(context, 4)));
        return dot;
    }

    /** Карточка раздела меню. */
    public static LinearLayout menuCard(Context context, String icon, String title,
                                        String subtitle, View.OnClickListener listener) {
        LinearLayout card = card(context);
        card.setClickable(true);
        card.setFocusable(true);
        card.setOnClickListener(listener);
        LinearLayout row = row(context);
        TextView iconView = text(context, icon, 23, GREEN, Typeface.NORMAL);
        iconView.setGravity(Gravity.CENTER);
        iconView.setWidth(dp(context, 42));
        row.addView(iconView);
        LinearLayout textColumn = column(context);
        TextView titleView = text(context, title, 17, TEXT, Typeface.BOLD);
        textColumn.addView(titleView);
        TextView subtitleView = muted(context, subtitle);
        subtitleView.setPadding(0, dp(context, 2), 0, 0);
        textColumn.addView(subtitleView);
        LinearLayout.LayoutParams textParams = new LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f);
        row.addView(textColumn, textParams);
        row.addView(text(context, "›", 24, MUTED, Typeface.NORMAL));
        card.addView(row);
        return card;
    }

    /** Отступ между блоками. */
    public static View gap(Context context, int dpValue) {
        View gap = new View(context);
        gap.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, dp(context, dpValue)));
        return gap;
    }
}
