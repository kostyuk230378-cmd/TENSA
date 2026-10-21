package com.tensorlogix.tensa;

import android.Manifest;
import android.content.pm.PackageManager;
import android.media.AudioFormat;
import android.media.AudioRecord;
import android.media.MediaRecorder;
import android.os.Bundle;
import android.speech.RecognitionListener;
import android.speech.RecognizerIntent;
import android.speech.SpeechRecognizer;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;

/**
 * Мобильный чат Tensa: пузыри сообщений, голосовой ввод через системный
 * распознаватель Android и прямой резерв OpenRouter, если ядро на ПК
 * недоступно.
 */
public class ChatActivity extends android.app.Activity {

    /** Код запроса разрешения микрофона. */
    private static final int REQUEST_AUDIO = 41;

    /** Лента сообщений. */
    private LinearLayout messages;
    /** Прокрутка ленты. */
    private ScrollView scroll;
    /** Поле ввода. */
    private EditText input;
    /** Маленький овал состояния голоса. */
    private TextView voiceState;
    /** Системное распознавание речи. */
    private SpeechRecognizer recognizer;
    /** Идёт ли прослушивание. */
    private boolean listening;
    /** Запись WAV для ASR на ПК (fallback без системного распознавателя). */
    private AudioRecord audioRecord;
    private Thread audioThread;
    private ByteArrayOutputStream audioBytes;
    private volatile boolean serverRecording;
    /** Частота записи для ASR-сервера. */
    private static final int SAMPLE_RATE = 16000;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Ui.style(this);
        getWindow().setSoftInputMode(android.view.WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);

        LinearLayout root = Ui.column(this);
        root.setPadding(Ui.dp(this, 16), Ui.dp(this, 32),
                Ui.dp(this, 16), Ui.dp(this, 12));

        LinearLayout header = Ui.row(this);
        TextView back = Ui.text(this, "‹", 34, Ui.CYAN, android.graphics.Typeface.NORMAL);
        back.setGravity(Gravity.CENTER);
        back.setPadding(0, 0, Ui.dp(this, 12), 0);
        back.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                finish();
            }
        });
        header.addView(back);
        LinearLayout titles = Ui.column(this);
        titles.addView(Ui.text(this, "Tensa", 23, Ui.TEXT, android.graphics.Typeface.BOLD));
        titles.addView(Ui.muted(this, "секретарь на вашем ПК"));
        header.addView(titles);
        root.addView(header);

        scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        messages = Ui.column(this);
        messages.setPadding(0, Ui.dp(this, 14), 0, Ui.dp(this, 10));
        scroll.addView(messages);
        root.addView(scroll, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));

        voiceState = Ui.text(this, "● СЛУШАЮ…", 10, Ui.RED,
                android.graphics.Typeface.BOLD);
        voiceState.setGravity(Gravity.CENTER);
        voiceState.setPadding(Ui.dp(this, 10), Ui.dp(this, 5),
                Ui.dp(this, 10), Ui.dp(this, 5));
        voiceState.setBackground(Ui.box(Ui.CARD, Ui.RED, Ui.dp(this, 20)));
        voiceState.setVisibility(View.GONE);
        LinearLayout.LayoutParams voiceParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        voiceParams.gravity = Gravity.CENTER_HORIZONTAL;
        voiceParams.bottomMargin = Ui.dp(this, 8);
        root.addView(voiceState, voiceParams);

        LinearLayout composer = Ui.row(this);
        input = Ui.field(this, "Спросите Tensa…", "", InputType.TYPE_CLASS_TEXT
                | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        input.setMaxLines(4);
        input.setImeOptions(EditorInfo.IME_ACTION_SEND);
        input.setOnEditorActionListener(new TextView.OnEditorActionListener() {
            @Override
            public boolean onEditorAction(TextView view, int actionId, android.view.KeyEvent event) {
                if (actionId == EditorInfo.IME_ACTION_SEND) {
                    sendFromInput();
                    return true;
                }
                return false;
            }
        });
        composer.addView(input, new LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f));

        final TextView mic = Ui.text(this, "🎤", 20, Ui.CYAN, android.graphics.Typeface.NORMAL);
        mic.setGravity(Gravity.CENTER);
        mic.setBackground(Ui.box(Ui.CARD, Ui.BORDER, Ui.dp(this, 14)));
        LinearLayout.LayoutParams micParams = new LinearLayout.LayoutParams(
                Ui.dp(this, 50), Ui.dp(this, 50));
        micParams.leftMargin = Ui.dp(this, 8);
        composer.addView(mic, micParams);
        mic.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                startVoice();
            }
        });

        TextView send = Ui.button(this, "→", Ui.GREEN, Ui.BG);
        LinearLayout.LayoutParams sendParams = new LinearLayout.LayoutParams(
                Ui.dp(this, 50), Ui.dp(this, 50));
        sendParams.leftMargin = Ui.dp(this, 8);
        composer.addView(send, sendParams);
        send.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View view) {
                sendFromInput();
            }
        });
        root.addView(composer);

        setContentView(root);
        addWelcome();
        initSpeech();
    }

    /** Приветствие в пустом чате. */
    private void addWelcome() {
        TextView hello = Ui.muted(this,
                "Спросите Tensa о документах, почте или планах. "
                        + "Голосовой ввод — кнопка с микрофоном справа.");
        hello.setTextSize(13);
        hello.setLineSpacing(0, 1.3f);
        hello.setPadding(Ui.dp(this, 6), Ui.dp(this, 8), Ui.dp(this, 6), Ui.dp(this, 12));
        messages.addView(hello);
    }

    /** Отправить текст из поля. */
    private void sendFromInput() {
        String text = input.getText().toString().trim();
        if (text.isEmpty()) {
            return;
        }
        input.setText("");
        sendText(text);
    }

    /** Пузырь пользователя и ответ ядра/облака. */
    private void sendText(final String text) {
        addBubble(text, true);
        final TextView pending = addBubble("Tensa печатает…", false);
        pending.setTextColor(Ui.MUTED);

        Background.run(new Background.Task<String>() {
            @Override
            public String run() throws Exception {
                return ask(text);
            }
        }, new Background.Callback<String>() {
            @Override
            public void onResult(String value) {
                pending.setTextColor(Ui.TEXT);
                pending.setText(value);
                scrollToBottom();
            }

            @Override
            public void onError(Exception error) {
                pending.setTextColor(Ui.RED);
                pending.setText("Ошибка: " + error.getMessage());
                scrollToBottom();
            }
        });
    }

    /** Ядро, при ошибке — прямой OpenRouter, если пользователь дал ключ. */
    private String ask(String text) throws Exception {
        String server = Prefs.server(ChatActivity.this);
        String bearer = Prefs.coreToken(ChatActivity.this);
        String coreError = null;
        if (!server.isEmpty() && !bearer.isEmpty()) {
            try {
                JSONObject response = CoreClient.query(server, bearer, text);
                String answer = response.optString("text", "");
                if (!answer.isEmpty()) {
                    return answer;
                }
                coreError = "пустой ответ ядра";
            } catch (Exception error) {
                coreError = error.getMessage();
            }
        }
        String key = Prefs.openRouterKey(ChatActivity.this);
        if (!key.isEmpty()) {
            return OpenRouterClient.chat(key, Prefs.model(ChatActivity.this), text);
        }
        if (coreError != null) {
            throw new Exception("ПК: " + coreError + ". Ключ OpenRouter не задан.");
        }
        throw new Exception("Подключите ПК в Настройках или добавьте ключ OpenRouter.");
    }

    /** Добавить пузырь и прокрутить. */
    private TextView addBubble(String text, boolean user) {
        TextView bubble = new TextView(this);
        bubble.setText(text);
        bubble.setTextSize(15);
        bubble.setTextColor(Ui.TEXT);
        bubble.setLineSpacing(0, 1.25f);
        bubble.setPadding(Ui.dp(this, 13), Ui.dp(this, 10),
                Ui.dp(this, 13), Ui.dp(this, 10));
        bubble.setBackground(Ui.box(user ? 0xFF0C2A2D : Ui.CARD,
                user ? Ui.CYAN : Ui.BORDER, Ui.dp(this, 15)));
        bubble.setMaxWidth((int) (getResources().getDisplayMetrics().widthPixels * 0.86f));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        params.gravity = user ? Gravity.END : Gravity.START;
        params.topMargin = Ui.dp(this, 6);
        messages.addView(bubble, params);
        scrollToBottom();
        return bubble;
    }

    /** Прокрутить ленту вниз. */
    private void scrollToBottom() {
        scroll.post(new Runnable() {
            @Override
            public void run() {
                scroll.fullScroll(View.FOCUS_DOWN);
            }
        });
    }

    /** Создать системный распознаватель речи. */
    private void initSpeech() {
        if (!SpeechRecognizer.isRecognitionAvailable(this)) {
            return;
        }
        recognizer = SpeechRecognizer.createSpeechRecognizer(this);
        recognizer.setRecognitionListener(new RecognitionListener() {
            @Override
            public void onReadyForSpeech(Bundle params) {
                setVoiceState("● СЛУШАЮ…", Ui.RED);
            }

            @Override
            public void onBeginningOfSpeech() {
                setVoiceState("● СЛУШАЮ…", Ui.RED);
            }

            @Override
            public void onRmsChanged(float rmsdB) {
            }

            @Override
            public void onBufferReceived(byte[] buffer) {
            }

            @Override
            public void onEndOfSpeech() {
                setVoiceState("● РАСПОЗНАЮ…", Ui.CYAN);
            }

            @Override
            public void onError(int error) {
                listening = false;
                setVoiceState("", Ui.MUTED);
                voiceState.setVisibility(View.GONE);
                Toast.makeText(ChatActivity.this, speechError(error), Toast.LENGTH_SHORT).show();
            }

            @Override
            public void onResults(Bundle results) {
                listening = false;
                ArrayList<String> matches = results.getStringArrayList(
                        SpeechRecognizer.RESULTS_RECOGNITION);
                setVoiceState("● РАСПОЗНАЮ…", Ui.CYAN);
                if (matches != null && !matches.isEmpty()) {
                    String text = matches.get(0).trim();
                    input.setText(text);
                    voiceState.setVisibility(View.GONE);
                    if (!text.isEmpty()) {
                        sendText(text);
                        input.setText("");
                    }
                } else {
                    voiceState.setVisibility(View.GONE);
                }
            }

            @Override
            public void onPartialResults(Bundle partialResults) {
                ArrayList<String> matches = partialResults.getStringArrayList(
                        SpeechRecognizer.RESULTS_RECOGNITION);
                if (matches != null && !matches.isEmpty()) {
                    input.setText(matches.get(0));
                    input.setSelection(input.getText().length());
                }
            }

            @Override
            public void onEvent(int eventType, Bundle params) {
            }
        });
    }

    /** Начать или отменить прослушивание. */
    private void startVoice() {
        if (recognizer == null) {
            startServerVoice();
            return;
        }
        if (listening) {
            recognizer.cancel();
            listening = false;
            voiceState.setVisibility(View.GONE);
            return;
        }
        if (serverRecording) {
            stopServerVoice();
            return;
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO)
                != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.RECORD_AUDIO}, REQUEST_AUDIO);
            return;
        }
        listening = true;
        setVoiceState("● СЛУШАЮ…", Ui.RED);
        android.content.Intent intent = new android.content.Intent(
                RecognizerIntent.ACTION_RECOGNIZE_SPEECH);
        intent.putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                RecognizerIntent.LANGUAGE_MODEL_FREE_FORM);
        intent.putExtra(RecognizerIntent.EXTRA_LANGUAGE, "ru-RU");
        intent.putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true);
        intent.putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1);
        recognizer.startListening(intent);
    }

    /** Записать WAV и отправить в ASR на ПК (без Google-распознавателя). */
    private void startServerVoice() {
        if (serverRecording) {
            stopServerVoice();
            return;
        }
        String server = Prefs.server(this);
        String bearer = Prefs.coreToken(this);
        if (server.isEmpty() || bearer.isEmpty()) {
            Toast.makeText(this, "Для голоса подключите ПК в Настройках.",
                    Toast.LENGTH_LONG).show();
            return;
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO)
                != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.RECORD_AUDIO}, REQUEST_AUDIO);
            return;
        }
        try {
            int min = AudioRecord.getMinBufferSize(SAMPLE_RATE,
                    AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT);
            if (min <= 0) {
                throw new IllegalStateException("микрофон не поддерживает 16 кГц");
            }
            audioRecord = new AudioRecord(MediaRecorder.AudioSource.MIC, SAMPLE_RATE,
                    AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT,
                    Math.max(min, SAMPLE_RATE));
            if (audioRecord.getState() != AudioRecord.STATE_INITIALIZED) {
                throw new IllegalStateException("не удалось открыть микрофон");
            }
            audioBytes = new ByteArrayOutputStream();
            serverRecording = true;
            setVoiceState("● СЛУШАЮ…", Ui.RED);
            audioRecord.startRecording();
            audioThread = new Thread(new Runnable() {
                @Override
                public void run() {
                    byte[] buffer = new byte[3200];
                    while (serverRecording) {
                        int read = audioRecord.read(buffer, 0, buffer.length);
                        if (read > 0) {
                            audioBytes.write(buffer, 0, read);
                        } else if (read < 0) {
                            break;
                        }
                    }
                }
            }, "tensa-wav-mic");
            audioThread.start();
        } catch (Exception error) {
            stopServerVoice();
            Toast.makeText(this, "Микрофон: " + error.getMessage(), Toast.LENGTH_LONG).show();
        }
    }

    /** Остановить запись и распознать WAV на ПК. */
    private void stopServerVoice() {
        if (!serverRecording) {
            return;
        }
        serverRecording = false;
        try {
            if (audioRecord != null) {
                audioRecord.stop();
            }
        } catch (Exception ignored) {
            // микрофон уже остановлен
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
        byte[] pcm = audioBytes == null ? new byte[0] : audioBytes.toByteArray();
        audioBytes = null;
        if (pcm.length < SAMPLE_RATE / 5) {
            setVoiceState("", Ui.MUTED);
            Toast.makeText(this, "Слишком короткая запись.", Toast.LENGTH_SHORT).show();
            return;
        }
        setVoiceState("● РАСПОЗНАЮ…", Ui.CYAN);
        final String server = Prefs.server(this);
        final String bearer = Prefs.coreToken(this);
        final byte[] wav = wavFromPcm(pcm);
        Background.run(new Background.Task<String>() {
            @Override
            public String run() throws Exception {
                return CoreClient.transcribeAudio(server, bearer, wav);
            }
        }, new Background.Callback<String>() {
            @Override
            public void onResult(String value) {
                setVoiceState("", Ui.MUTED);
                sendText(value);
            }

            @Override
            public void onError(Exception error) {
                setVoiceState("", Ui.MUTED);
                Toast.makeText(ChatActivity.this,
                        "ASR на ПК: " + error.getMessage(), Toast.LENGTH_LONG).show();
            }
        });
    }

    /** PCM16 mono 16 кГц → WAV. */
    private byte[] wavFromPcm(byte[] pcm) {
        int total = 36 + pcm.length;
        ByteBuffer buffer = ByteBuffer.allocate(44 + pcm.length).order(ByteOrder.LITTLE_ENDIAN);
        buffer.put("RIFF".getBytes(StandardCharsets.US_ASCII));
        buffer.putInt(total);
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

    /** Показать/скрыть овал состояния. */
    private void setVoiceState(String text, int color) {
        if (text == null || text.isEmpty()) {
            voiceState.setVisibility(View.GONE);
            return;
        }
        voiceState.setText(text);
        voiceState.setTextColor(color);
        voiceState.setBackground(Ui.box(Ui.CARD, color, Ui.dp(this, 20)));
        voiceState.setVisibility(View.VISIBLE);
    }

    /** Понятная ошибка распознавания. */
    private String speechError(int error) {
        switch (error) {
            case SpeechRecognizer.ERROR_AUDIO:
                return "Микрофон недоступен.";
            case SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS:
                return "Разрешите микрофон в настройках Android.";
            case SpeechRecognizer.ERROR_NETWORK:
            case SpeechRecognizer.ERROR_NETWORK_TIMEOUT:
                return "Распознавание без сети недоступно на этом телефоне.";
            case SpeechRecognizer.ERROR_NO_MATCH:
                return "Не расслышал — попробуйте ещё раз.";
            case SpeechRecognizer.ERROR_RECOGNIZER_BUSY:
                return "Распознаватель занят.";
            case SpeechRecognizer.ERROR_SPEECH_TIMEOUT:
                return "Не дождался речи.";
            default:
                return "Не удалось распознать речь.";
        }
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(requestCode, permissions, results);
        if (requestCode == REQUEST_AUDIO && results.length > 0
                && results[0] == PackageManager.PERMISSION_GRANTED) {
            startVoice();
        }
    }

    @Override
    protected void onDestroy() {
        if (serverRecording) {
            stopServerVoice();
        }
        if (recognizer != null) {
            recognizer.destroy();
            recognizer = null;
        }
        super.onDestroy();
    }
}
