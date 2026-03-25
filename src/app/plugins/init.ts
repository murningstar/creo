import { usePlatformStore } from '~/entities/platform';
import { useSettingsStore } from '~/entities/settings';
import { useAudioStore } from '~/entities/audio';

export default defineNuxtPlugin(async () => {
    const platformStore = usePlatformStore();

    if (platformStore.isNativePlatform) {
        const settingsStore = useSettingsStore();
        const audioStore = useAudioStore();

        await settingsStore.init();
        audioStore.checkModels();
        audioStore.setupEventListeners();
    }
});
