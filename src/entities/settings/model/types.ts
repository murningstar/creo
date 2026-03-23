export type TextInputMethod = 'paste' | 'type';

export interface AppSettings {
    textInputMethod: TextInputMethod;
    historyRetentionDays: number;
}

export const DEFAULT_SETTINGS: AppSettings = {
    textInputMethod: 'paste',
    historyRetentionDays: 30,
};
