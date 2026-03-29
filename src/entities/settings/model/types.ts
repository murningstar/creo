import type { KeyCombo } from '~/shared/ui/keystroke-recorder/model/types';

export type TextInputMethod = 'paste' | 'type';
export type HotkeyMode = 'hold' | 'toggle';
export type SttEngine = 'auto' | 'parakeet' | 'whisper';

export const DEFAULT_ASSISTANT_NAME = 'Крео';

export const DEFAULT_HOTKEY: KeyCombo = {
    key: '`',
    code: 'Backquote',
    ctrl: true,
    alt: false,
    shift: false,
    meta: false,
};

export interface AppSettings {
    assistantName: string;
    textInputMethod: TextInputMethod;
    historyRetentionDays: number;
    hotkey: KeyCombo | null;
    hotkeyMode: HotkeyMode;
    sttEngine: SttEngine;
}

/** Store keys — single source of truth for Tauri Store key names. */
export const STORE_KEY = {
    assistantName: 'assistantName',
    textInputMethod: 'textInputMethod',
    historyRetentionDays: 'historyRetentionDays',
    hotkey: 'hotkey',
    hotkeyMode: 'hotkeyMode',
    sttEngine: 'sttEngine',
} as const satisfies Record<keyof AppSettings, string>;

export const STORE_FILENAME = 'settings.json';

export const DEFAULT_SETTINGS: AppSettings = {
    assistantName: DEFAULT_ASSISTANT_NAME,
    textInputMethod: 'paste',
    historyRetentionDays: 30,
    hotkey: DEFAULT_HOTKEY,
    hotkeyMode: 'hold',
    sttEngine: 'auto',
};
