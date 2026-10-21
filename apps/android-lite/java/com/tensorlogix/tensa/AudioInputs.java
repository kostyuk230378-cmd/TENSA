package com.tensorlogix.tensa;

import android.content.Context;
import android.media.AudioDeviceInfo;
import android.media.AudioManager;
import android.media.AudioRecord;
import android.media.MediaRecorder;
import android.os.Build;

import java.util.ArrayList;
import java.util.List;

/**
 * Микрофоны Android: встроенный, USB, Bluetooth, проводная гарнитура.
 *
 * Список берётся у системы через AudioManager.getDevices(GET_DEVICES_INPUTS).
 * Выбор пользователя хранится по AudioDeviceInfo.getId(); при записи он
 * применяется через AudioRecord.setPreferredDevice. Если устройство
 * отключили, запись автоматически идёт с системного микрофона.
 */
public final class AudioInputs {

    /** Один доступный системе вход. */
    public static final class Device {
        /** Идентификатор AudioDeviceInfo. */
        public final int id;
        /** Понятное человеку имя. */
        public final String name;
        /** Тип AudioDeviceInfo.TYPE_*. */
        public final int type;

        Device(int id, String name, int type) {
            this.id = id;
            this.name = name;
            this.type = type;
        }

        /** Подпись для списка настроек. */
        public String label() {
            String typeLabel = typeName(type);
            if (name.equalsIgnoreCase(typeLabel)) {
                return name;
            }
            return name + " · " + typeLabel;
        }

        /** Bluetooth-микрофон требует отдельного режима связи. */
        public boolean isBluetooth() {
            return type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO;
        }
    }

    private AudioInputs() {
    }

    /** Все входы, которые сейчас видит Android. */
    public static List<Device> devices(Context context) {
        List<Device> result = new ArrayList<>();
        AudioManager manager = (AudioManager) context.getSystemService(Context.AUDIO_SERVICE);
        if (manager == null) {
            return result;
        }
        AudioDeviceInfo[] inputs;
        try {
            inputs = manager.getDevices(AudioManager.GET_DEVICES_INPUTS);
        } catch (Exception error) {
            return result;
        }
        for (AudioDeviceInfo input : inputs) {
            if (input == null) {
                continue;
            }
            CharSequence product = input.getProductName();
            String name = product == null ? "" : product.toString().trim();
            if (name.isEmpty() || "Android".equalsIgnoreCase(name)) {
                name = typeName(input.getType());
            }
            result.add(new Device(input.getId(), name, input.getType()));
        }
        return result;
    }

    /** Имя типа входа для человека. */
    public static String typeName(int type) {
        switch (type) {
            case AudioDeviceInfo.TYPE_BUILTIN_MIC:
                return "встроенный";
            case AudioDeviceInfo.TYPE_USB_DEVICE:
                return "USB";
            case AudioDeviceInfo.TYPE_USB_HEADSET:
                return "USB-гарнитура";
            case AudioDeviceInfo.TYPE_BLUETOOTH_SCO:
                return "Bluetooth";
            case AudioDeviceInfo.TYPE_WIRED_HEADSET:
                return "гарнитура 3,5 мм";
            case AudioDeviceInfo.TYPE_LINE_ANALOG:
                return "линейный вход";
            case AudioDeviceInfo.TYPE_LINE_DIGITAL:
                return "цифровой вход";
            case AudioDeviceInfo.TYPE_TELEPHONY:
                return "телефония";
            case AudioDeviceInfo.TYPE_FM_TUNER:
                return "FM-тюнер";
            case AudioDeviceInfo.TYPE_TV_TUNER:
                return "ТВ-тюнер";
            case AudioDeviceInfo.TYPE_HDMI:
                return "HDMI";
            case AudioDeviceInfo.TYPE_REMOTE_SUBMIX:
                return "системный микс";
            default:
                return "вход Android";
        }
    }

    /** Выбранный пользователем вход или null для системного. */
    public static Device selected(Context context) {
        int wanted = Prefs.micId(context);
        if (wanted == 0) {
            return null;
        }
        for (Device device : devices(context)) {
            if (device.id == wanted) {
                return device;
            }
        }
        return null;
    }

    /** Источник записи: Bluetooth Sco идёт через режим связи, остальные — MIC. */
    public static int source(Context context) {
        Device device = selected(context);
        if (device != null && device.isBluetooth()) {
            return MediaRecorder.AudioSource.VOICE_COMMUNICATION;
        }
        return MediaRecorder.AudioSource.MIC;
    }

    /**
     * Направить уже созданный AudioRecord на выбранный вход.
     *
     * @return true, если выбранный вход найден и система приняла маршрут.
     */
    public static boolean apply(AudioRecord record, Context context) {
        Device wanted = selected(context);
        if (wanted == null || record == null) {
            return false;
        }
        AudioManager manager = (AudioManager) context.getSystemService(Context.AUDIO_SERVICE);
        if (manager == null) {
            return false;
        }
        for (AudioDeviceInfo input : manager.getDevices(AudioManager.GET_DEVICES_INPUTS)) {
            if (input.getId() != wanted.id) {
                continue;
            }
            try {
                if (wanted.isBluetooth()) {
                    manager.setMode(AudioManager.MODE_IN_COMMUNICATION);
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        manager.setCommunicationDevice(input);
                    } else {
                        manager.startBluetoothSco();
                        manager.setBluetoothScoOn(true);
                    }
                }
                return record.setPreferredDevice(input);
            } catch (Exception ignored) {
                return false;
            }
        }
        return false;
    }

    /** Вернуть звук телефона в обычный режим после Bluetooth-записи. */
    public static void release(Context context) {
        AudioManager manager = (AudioManager) context.getSystemService(Context.AUDIO_SERVICE);
        if (manager == null) {
            return;
        }
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                manager.clearCommunicationDevice();
            } else {
                manager.stopBluetoothSco();
                manager.setBluetoothScoOn(false);
            }
            manager.setMode(AudioManager.MODE_NORMAL);
        } catch (Exception ignored) {
            // производитель мог запретить смену режима — запись уже завершена
        }
    }
}
