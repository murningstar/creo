import { platform as getPlatform } from '@tauri-apps/plugin-os';
import type { CurrentNativePlatform } from '../model/types';

export const usePlatformStore = defineStore('platform', () => {
    const _currentNativePlatform = ref<CurrentNativePlatform>(null);

    try {
        _currentNativePlatform.value = getPlatform();
    } catch {
        console.info('App is not in native mode');
    }

    const currentNativePlatform = readonly(_currentNativePlatform);

    const isNativePlatform = computed<boolean>(() => currentNativePlatform.value !== null);

    const isNativeDesktop = computed<boolean>(
        () => isNativePlatform.value && ['windows', 'linux', 'macos'].includes(_currentNativePlatform.value ?? '')
    );

    const isWebBrowser = computed<boolean>(() => !isNativePlatform.value);

    const platformLabel = computed(() =>
        isNativePlatform.value ? `Platform: ${currentNativePlatform.value}` : 'Platform: Web Browser'
    );

    return {
        currentNativePlatform,
        isNativePlatform,
        isNativeDesktop,
        isWebBrowser,
        platformLabel,
    };
});
