import { load, type Store } from '@tauri-apps/plugin-store';

import type { TextInputMethod } from '../model/types';
import { DEFAULT_SETTINGS } from '../model/types';

export const useSettingsStore = defineStore('settings', () => {
    const _store = ref<Store | null>(null);
    const _assistantName = ref<string>(DEFAULT_SETTINGS.assistantName);
    const _textInputMethod = ref<TextInputMethod>(DEFAULT_SETTINGS.textInputMethod);
    const _historyRetentionDays = ref<number>(DEFAULT_SETTINGS.historyRetentionDays);

    const assistantName = readonly(_assistantName);
    const textInputMethod = readonly(_textInputMethod);
    const historyRetentionDays = readonly(_historyRetentionDays);

    async function init() {
        const store = await load('settings.json', {
            autoSave: true,
            defaults: {
                assistantName: DEFAULT_SETTINGS.assistantName,
                textInputMethod: DEFAULT_SETTINGS.textInputMethod,
                historyRetentionDays: DEFAULT_SETTINGS.historyRetentionDays,
            },
        });
        _store.value = store;

        // Load persisted values (or use defaults)
        _assistantName.value = (await store.get<string>('assistantName')) ?? DEFAULT_SETTINGS.assistantName;
        _textInputMethod.value =
            (await store.get<TextInputMethod>('textInputMethod')) ?? DEFAULT_SETTINGS.textInputMethod;
        _historyRetentionDays.value =
            (await store.get<number>('historyRetentionDays')) ?? DEFAULT_SETTINGS.historyRetentionDays;

        // React to changes from Rust side or other windows
        await store.onKeyChange<string>('assistantName', value => {
            if (value != null) _assistantName.value = value;
        });
        await store.onKeyChange<TextInputMethod>('textInputMethod', value => {
            if (value != null) _textInputMethod.value = value;
        });
        await store.onKeyChange<number>('historyRetentionDays', value => {
            if (value != null) _historyRetentionDays.value = value;
        });
    }

    async function setAssistantName(name: string) {
        _assistantName.value = name;
        await _store.value?.set('assistantName', name);
        await _store.value?.save();
    }

    async function setTextInputMethod(method: TextInputMethod) {
        _textInputMethod.value = method;
        await _store.value?.set('textInputMethod', method);
        await _store.value?.save();
    }

    async function setHistoryRetentionDays(days: number) {
        _historyRetentionDays.value = days;
        await _store.value?.set('historyRetentionDays', days);
        await _store.value?.save();
    }

    return {
        assistantName,
        textInputMethod,
        historyRetentionDays,
        init,
        setAssistantName,
        setTextInputMethod,
        setHistoryRetentionDays,
    };
});
