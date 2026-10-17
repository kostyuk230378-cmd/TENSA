package com.tensorlogix.tensa;

/** Фоновый исполнитель запросов: сеть не должна жить в UI-потоке. */
public final class Background {

    private Background() {
    }

    /** Тело задачи для фонового потока. */
    public interface Task<T> {
        /**
         * Выполнить работу (в фоновом потоке).
         *
         * @return результат
         * @throws Exception любая ошибка — уйдёт в onError
         */
        T run() throws Exception;
    }

    /** Колбэк результата (в UI-потоке). */
    public interface Callback<T> {
        /** Успешный результат. */
        void onResult(T value);

        /** Ошибка выполнения. */
        void onError(Exception error);
    }

    /**
     * Выполнить задачу в фоне и вернуть результат в UI-поток.
     *
     * @param task работа
     * @param callback колбэк (может быть null, тогда ошибки игнорируются)
     */
    public static <T> void run(final Task<T> task, final Callback<T> callback) {
        final android.os.Handler handler = new android.os.Handler(
                android.os.Looper.getMainLooper());
        new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    final T value = task.run();
                    if (callback != null) {
                        handler.post(new Runnable() {
                            @Override
                            public void run() {
                                callback.onResult(value);
                            }
                        });
                    }
                } catch (final Exception error) {
                    if (callback != null) {
                        handler.post(new Runnable() {
                            @Override
                            public void run() {
                                callback.onError(error);
                            }
                        });
                    }
                }
            }
        }).start();
    }
}
