import type { KeyCombo } from '~/shared/ui/keystroke-recorder/model/types';

export type TextInputMethod = 'paste' | 'type';
export type HotkeyMode = 'hold' | 'toggle';

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
}

export const DEFAULT_SETTINGS: AppSettings = {
    assistantName: DEFAULT_ASSISTANT_NAME,
    textInputMethod: 'paste',
    historyRetentionDays: 30,
    hotkey: DEFAULT_HOTKEY,
    hotkeyMode: 'hold',
};
