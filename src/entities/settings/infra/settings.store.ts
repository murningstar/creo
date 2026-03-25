import { load, type Store } from '@tauri-apps/plugin-store';

import type { KeyCombo } from '~/shared/ui/keystroke-recorder/model/types';

import type { HotkeyMode, TextInputMethod } from '../model/types';
import { DEFAULT_SETTINGS } from '../model/types';

export const useSettingsStore = defineStore('settings', () => {
    const _store = ref<Store | null>(null);
    const _assistantName = ref<string>(DEFAULT_SETTINGS.assistantName);
    const _textInputMethod = ref<TextInputMethod>(DEFAULT_SETTINGS.textInputMethod);
    const _historyRetentionDays = ref<number>(DEFAULT_SETTINGS.historyRetentionDays);
    const _hotkey = ref<KeyCombo | null>(DEFAULT_SETTINGS.hotkey);
    const _hotkeyMode = ref<HotkeyMode>(DEFAULT_SETTINGS.hotkeyMode);

    const assistantName = readonly(_assistantName);
    const textInputMethod = readonly(_textInputMethod);
    const historyRetentionDays = readonly(_historyRetentionDays);
    const hotkey = readonly(_hotkey);
    const hotkeyMode = readonly(_hotkeyMode);

    async function init() {
        const store = await load('settings.json', {
            autoSave: true,
            defaults: {
                assistantName: DEFAULT_SETTINGS.assistantName,
                textInputMethod: DEFAULT_SETTINGS.textInputMethod,
                historyRetentionDays: DEFAULT_SETTINGS.historyRetentionDays,
                hotkey: DEFAULT_SETTINGS.hotkey,
                hotkeyMode: DEFAULT_SETTINGS.hotkeyMode,
            },
        });
        _store.value = store;

        // Load persisted values (or use defaults)
        _assistantName.value = (await store.get<string>('assistantName')) ?? DEFAULT_SETTINGS.assistantName;
        _textInputMethod.value =
            (await store.get<TextInputMethod>('textInputMethod')) ?? DEFAULT_SETTINGS.textInputMethod;
        _historyRetentionDays.value =
            (await store.get<number>('historyRetentionDays')) ?? DEFAULT_SETTINGS.historyRetentionDays;
        _hotkey.value = (await store.get<KeyCombo | null>('hotkey')) ?? DEFAULT_SETTINGS.hotkey;
        _hotkeyMode.value = (await store.get<HotkeyMode>('hotkeyMode')) ?? DEFAULT_SETTINGS.hotkeyMode;

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
        await store.onKeyChange<KeyCombo | null>('hotkey', value => {
            _hotkey.value = value ?? null;
        });
        await store.onKeyChange<HotkeyMode>('hotkeyMode', value => {
            if (value != null) _hotkeyMode.value = value;
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

    async function setHotkey(combo: KeyCombo | null) {
        _hotkey.value = combo;
        await _store.value?.set('hotkey', combo);
        await _store.value?.save();
    }

    async function setHotkeyMode(mode: HotkeyMode) {
        _hotkeyMode.value = mode;
        await _store.value?.set('hotkeyMode', mode);
        await _store.value?.save();
    }

    return {
        assistantName,
        textInputMethod,
        historyRetentionDays,
        hotkey,
        hotkeyMode,
        init,
        setAssistantName,
        setTextInputMethod,
        setHistoryRetentionDays,
        setHotkey,
        setHotkeyMode,
    };
});
