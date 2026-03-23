import { load, type Store } from '@tauri-apps/plugin-store';

import type { TextInputMethod } from '../model/types';
import { DEFAULT_SETTINGS } from '../model/types';

export const useSettingsStore = defineStore('settings', () => {
    const _store = ref<Store | null>(null);
    const _textInputMethod = ref<TextInputMethod>(DEFAULT_SETTINGS.textInputMethod);
    const _historyRetentionDays = ref<number>(DEFAULT_SETTINGS.historyRetentionDays);

    const textInputMethod = readonly(_textInputMethod);
    const historyRetentionDays = readonly(_historyRetentionDays);

    async function init() {
        const store = await load('settings.json', { autoSave: true });
        _store.value = store;

        // Load persisted values (or use defaults)
        _textInputMethod.value =
            (await store.get<TextInputMethod>('textInputMethod')) ?? DEFAULT_SETTINGS.textInputMethod;
        _historyRetentionDays.value =
            (await store.get<number>('historyRetentionDays')) ?? DEFAULT_SETTINGS.historyRetentionDays;

        // React to changes from Rust side or other windows
        await store.onKeyChange<TextInputMethod>('textInputMethod', value => {
            if (value != null) _textInputMethod.value = value;
        });
        await store.onKeyChange<number>('historyRetentionDays', value => {
            if (value != null) _historyRetentionDays.value = value;
        });
    }

    async function setTextInputMethod(method: TextInputMethod) {
        _textInputMethod.value = method;
        await _store.value?.set('textInputMethod', method);
    }

    async function setHistoryRetentionDays(days: number) {
        _historyRetentionDays.value = days;
        await _store.value?.set('historyRetentionDays', days);
    }

    return {
        textInputMethod,
        historyRetentionDays,
        init,
        setTextInputMethod,
        setHistoryRetentionDays,
    };
});
