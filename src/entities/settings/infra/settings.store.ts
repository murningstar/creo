import { load, type Store } from '@tauri-apps/plugin-store';

import type { KeyCombo } from '~/shared/ui/keystroke-recorder/model/types';

import type { HotkeyMode, SttEngine, TextInputMethod } from '../model/types';
import { DEFAULT_SETTINGS, STORE_FILENAME, STORE_KEY } from '../model/types';

export const useSettingsStore = defineStore('settings', () => {
    let _store: Store | null = null;
    const _assistantName = ref<string>(DEFAULT_SETTINGS.assistantName);
    const _textInputMethod = ref<TextInputMethod>(DEFAULT_SETTINGS.textInputMethod);
    const _historyRetentionDays = ref<number>(DEFAULT_SETTINGS.historyRetentionDays);
    const _hotkey = ref<KeyCombo | null>(DEFAULT_SETTINGS.hotkey);
    const _hotkeyMode = ref<HotkeyMode>(DEFAULT_SETTINGS.hotkeyMode);
    const _sttEngine = ref<SttEngine>(DEFAULT_SETTINGS.sttEngine);

    const assistantName = readonly(_assistantName);
    const textInputMethod = readonly(_textInputMethod);
    const historyRetentionDays = readonly(_historyRetentionDays);
    const hotkey = readonly(_hotkey);
    const hotkeyMode = readonly(_hotkeyMode);
    const sttEngine = readonly(_sttEngine);

    async function init() {
        const store = await load(STORE_FILENAME, {
            autoSave: false,
            defaults: {
                [STORE_KEY.assistantName]: DEFAULT_SETTINGS.assistantName,
                [STORE_KEY.textInputMethod]: DEFAULT_SETTINGS.textInputMethod,
                [STORE_KEY.historyRetentionDays]: DEFAULT_SETTINGS.historyRetentionDays,
                [STORE_KEY.hotkey]: DEFAULT_SETTINGS.hotkey,
                [STORE_KEY.hotkeyMode]: DEFAULT_SETTINGS.hotkeyMode,
                [STORE_KEY.sttEngine]: DEFAULT_SETTINGS.sttEngine,
            },
        });
        _store = store;

        _assistantName.value = (await store.get<string>(STORE_KEY.assistantName)) ?? DEFAULT_SETTINGS.assistantName;
        _textInputMethod.value =
            (await store.get<TextInputMethod>(STORE_KEY.textInputMethod)) ?? DEFAULT_SETTINGS.textInputMethod;
        _historyRetentionDays.value =
            (await store.get<number>(STORE_KEY.historyRetentionDays)) ?? DEFAULT_SETTINGS.historyRetentionDays;
        _hotkey.value = (await store.get<KeyCombo | null>(STORE_KEY.hotkey)) ?? DEFAULT_SETTINGS.hotkey;
        _hotkeyMode.value = (await store.get<HotkeyMode>(STORE_KEY.hotkeyMode)) ?? DEFAULT_SETTINGS.hotkeyMode;
        _sttEngine.value = (await store.get<SttEngine>(STORE_KEY.sttEngine)) ?? DEFAULT_SETTINGS.sttEngine;

        await store.onKeyChange<string>(STORE_KEY.assistantName, value => {
            if (value != null) _assistantName.value = value;
        });
        await store.onKeyChange<TextInputMethod>(STORE_KEY.textInputMethod, value => {
            if (value != null) _textInputMethod.value = value;
        });
        await store.onKeyChange<number>(STORE_KEY.historyRetentionDays, value => {
            if (value != null) _historyRetentionDays.value = value;
        });
        await store.onKeyChange<KeyCombo | null>(STORE_KEY.hotkey, value => {
            _hotkey.value = value ?? null;
        });
        await store.onKeyChange<HotkeyMode>(STORE_KEY.hotkeyMode, value => {
            if (value != null) _hotkeyMode.value = value;
        });
        await store.onKeyChange<SttEngine>(STORE_KEY.sttEngine, value => {
            if (value != null) _sttEngine.value = value;
        });
    }

    function _requireStore(): Store {
        if (!_store) {
            throw new Error('Settings store not initialized — call init() first');
        }
        return _store;
    }

    async function setAssistantName(name: string) {
        const store = _requireStore();
        _assistantName.value = name;
        await store.set(STORE_KEY.assistantName, name);
        await store.save();
    }

    async function setTextInputMethod(method: TextInputMethod) {
        const store = _requireStore();
        _textInputMethod.value = method;
        await store.set(STORE_KEY.textInputMethod, method);
        await store.save();
    }

    async function setHistoryRetentionDays(days: number) {
        const store = _requireStore();
        _historyRetentionDays.value = days;
        await store.set(STORE_KEY.historyRetentionDays, days);
        await store.save();
    }

    async function setHotkey(combo: KeyCombo | null) {
        const store = _requireStore();
        _hotkey.value = combo;
        await store.set(STORE_KEY.hotkey, combo);
        await store.save();
    }

    async function setHotkeyMode(mode: HotkeyMode) {
        const store = _requireStore();
        _hotkeyMode.value = mode;
        await store.set(STORE_KEY.hotkeyMode, mode);
        await store.save();
    }

    async function setSttEngine(engine: SttEngine) {
        const store = _requireStore();
        _sttEngine.value = engine;
        await store.set(STORE_KEY.sttEngine, engine);
        await store.save();
    }

    return {
        assistantName,
        textInputMethod,
        historyRetentionDays,
        hotkey,
        hotkeyMode,
        sttEngine,
        init,
        setAssistantName,
        setTextInputMethod,
        setHistoryRetentionDays,
        setHotkey,
        setHotkeyMode,
        setSttEngine,
    };
});
