import { getCurrentWindow } from '@tauri-apps/api/window';

import { usePlatformStore } from '~/entities/platform';
import { useSettingsStore } from '~/entities/settings';
import { useAudioStore } from '~/entities/audio';
import { useDictationFlow } from '~/features/dictation-flow';
import { formatForTauri } from '~/features/hotkey-recorder';

/** Check if we're running in the main dashboard window (not overlay). */
function isMainWindow(): boolean {
    try {
        return getCurrentWindow().label === 'main';
    } catch {
        return true; // Fallback: assume main if Tauri API unavailable
    }
}

export default defineNuxtPlugin(async () => {
    const platformStore = usePlatformStore();

    // Skip full initialization in overlay window — it only listens to events
    if (platformStore.isNativePlatform && isMainWindow()) {
        const settingsStore = useSettingsStore();
        const audioStore = useAudioStore();

        await settingsStore.init();
        await audioStore.checkModels();
        audioStore.setupEventListeners();

        // Sync frontend with current pipeline state (handles webview reload)
        await audioStore.syncMode();

        // Auto-start: if all models present and pipeline not already running, go to Standby
        if (audioStore.isOff && audioStore.modelStatus?.allPresent) {
            audioStore.startListening(settingsStore.sttEngine);
        }

        // Register persisted hotkey (overrides Rust default if different)
        await registerGlobalHotkey(settingsStore.hotkey);

        const dictationFlow = useDictationFlow();
        dictationFlow.setupListeners();
    }
});

async function registerGlobalHotkey(hotkey: ReturnType<typeof useSettingsStore>['hotkey']['value']) {
    try {
        const { unregister, register } = await import('@tauri-apps/plugin-global-shortcut');
        const { getCurrentWindow } = await import('@tauri-apps/api/window');

        // Clear Rust-registered default hotkey (Ctrl+Backquote)
        try {
            await unregister('CmdOrCtrl+Backquote');
        } catch {
            // May not be registered — ignore
        }

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
