import { usePlatformStore } from '~/entities/platform';
import { useSettingsStore } from '~/entities/settings';
import { useAudioStore } from '~/entities/audio';
import { useDictationFlow } from '~/features/dictation-flow';
import { formatForTauri } from '~/features/hotkey-recorder';

export default defineNuxtPlugin(async () => {
    const platformStore = usePlatformStore();

    if (platformStore.isNativePlatform) {
        const settingsStore = useSettingsStore();
        const audioStore = useAudioStore();

        await settingsStore.init();
        audioStore.checkModels();
        audioStore.setupEventListeners();

        // Register persisted hotkey (overrides Rust default if different)
        await registerGlobalHotkey(settingsStore.hotkey);

        const dictationFlow = useDictationFlow();
        dictationFlow.setupListeners();
    }
});

async function registerGlobalHotkey(hotkey: ReturnType<typeof useSettingsStore>['hotkey']['value']) {
    try {
        const { unregisterAll, register } = await import('@tauri-apps/plugin-global-shortcut');
        const { getCurrentWindow } = await import('@tauri-apps/api/window');

        // Clear Rust-registered default
        await unregisterAll();

        if (!hotkey) return;

        const appWindow = getCurrentWindow();
        const shortcutStr = formatForTauri(hotkey);
        await register(shortcutStr, event => {
            if (event.state === 'Pressed') {
                appWindow.emit('hotkey-pressed');
            } else if (event.state === 'Released') {
                appWindow.emit('hotkey-released');
            }
        });
    } catch (e) {
        console.warn('Failed to register global hotkey:', e);
    }
}
