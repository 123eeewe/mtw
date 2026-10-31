package ru.mousetweaks_rs.host;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.loader.api.FabricLoader;

/**
 * Java-адаптер MouseTweaks-RS для Horizon/Fabric.
 *
 * Загружает нативную библиотеку (libmousetweaks_rs.so / mousetweaks_rs.dll),
 * инициализирует конфиг и предоставляет нативные методы, которые вызываются
 * из миксинов на HandledScreen.
 *
 * Миксины (см. MouseTweaksRsMixins.java) должны:
 *  - в mouseScrolled(double x, double y, double amount): собрать снимок слотов,
 *    вызвать {@link #scroll}, по коду действия выполнить clickSlot или отменить событие;
 *  - в mouseClicked/mouseReleased: использовать press/release/dragMove;
 *  - в slotUpdate/quickMove (toss): вызывать {@link #tossAttempt}.
 */
public final class MouseTweaksRs implements ClientModInitializer {

    public static final String MOD_ID = "mousetweaks_rs";

    // ---- native ----
    static {
        // В реальном сборочном пайплайне .dll/.so кладётся в ресурсы мода
        // и извлекается через FabricLoader; здесь — упрощённо.
        System.loadLibrary("mousetweaks_rs");
    }

    private static native void mtInit(String gameRoot);
    /** @return упакованный i64: [code:16][slot:32][amount:16] */
    private static native long mtScroll(double mx, double my, int rawUp, int ctrl,
                                        String cursorItem, int cursorCount, int cursorMax,
                                        int slotCount, int[] slotIndices, String[] slotItems,
                                        int[] slotCounts, double[] xs, double[] ys, double[] wh);
    private static native int mtPress(double mx, double my, int button, int ctrl, int overSlot);
    private static native long mtDragMove(double mx, double my, int overSlot);
    private static native long mtRelease(double mx, double my, int button, int overSlot);
    private static native int mtTossAttempt(int wholeStack);
    private static native void mtReloadConfig(String root);

    // ---- unpack helpers (bit layout must match Rust ffi::pack/unpack) ----
    public static int code(long packed)   { return (int) ((packed >>> 48) & 0xFFFF); }
    public static int slot(long packed)   { return (int) ((packed >>> 16) & 0xFFFFFFFFL); }
    public static int amount(long packed) { return (int) (packed & 0xFFFF); }

    // action codes (mirror of Rust ffi::code)
    public static final int PASS = 0, CANCEL = 1, GIVE_TO_CURSOR = 2, TAKE_FROM_CURSOR = 3,
            HALF_STACK = 4, WHOLE_STACK = 5, TOSS = 6;

    @Override
    public void onInitializeClient() {
        String root = FabricLoader.getInstance().getGameDir().toString();
        mtInit(root);
        ClientLifecycleEvents.CLIENT_STOPPING.register(client -> { /* nothing to unhook in GLFW path */ });
    }

    /** Обёртка для миксина mouseScrolled. */
    public static long scroll(double mx, double my, double amount, boolean ctrlHeld,
                              String cursorItem, int cursorCount, int cursorMax,
                              int[] indices, String[] items, int[] counts,
                              double[] xs, double[] ys, double[] wh) {
        return mtScroll(mx, my, amount > 0 ? 1 : 0, ctrlHeld ? 1 : 0,
                cursorItem, cursorCount, cursorMax,
                indices.length, indices, items, counts, xs, ys, wh);
    }

    public static int press(double mx, double my, int button, boolean ctrlHeld, int overSlot) {
        return mtPress(mx, my, button, ctrlHeld ? 1 : 0, overSlot);
    }

    public static long dragMove(double mx, double my, int overSlot) {
        return mtDragMove(mx, my, overSlot);
    }

    public static long release(double mx, double my, int button, int overSlot) {
        return mtRelease(mx, my, button, overSlot);
    }

    /** true — выброс стака надо отменить (предмет остаётся в руках). */
    public static boolean cancelToss(boolean wholeStack) {
        return mtTossAttempt(wholeStack ? 1 : 0) == 1;
    }

    public static void reloadConfig() {
        mtReloadConfig(FabricLoader.getInstance().getGameDir().toString());
    }
}
