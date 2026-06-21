package com.voxelpopuli.client;

public class GameLoop {
    public interface GameCallback {
        void onUpdate(float dt);
        void onRender(float alpha);
    }

    private final GameCallback callback;
    private boolean running = false;

    public GameLoop(GameCallback callback) {
        this.callback = callback;
    }

    public void start() {
        running = true;
        run();
    }

    public void stop() {
        running = false;
    }

    private void run() {
        double timeSec = getSystemTime();
        double tickTimeSec = timeSec;
        final double secondsPerTick = 0.05; // 50ms per tick

        while (running) {
            double now = getSystemTime();
            double elapsed = now - timeSec;
            timeSec = now;

            if (elapsed > 1.0) {
                elapsed = 1.0;
            }

            // Fixed logic ticking at 20 TPS
            while (now - tickTimeSec >= secondsPerTick) {
                callback.onUpdate((float) secondsPerTick);
                tickTimeSec += secondsPerTick;
            }

            float alpha = (float) ((now - tickTimeSec) / secondsPerTick);
            callback.onRender(alpha);

            try {
                Thread.sleep(1);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                break;
            }
        }
    }

    private double getSystemTime() {
        return System.nanoTime() / 1_000_000_000.0;
    }
}
