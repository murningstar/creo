export type TextInputMethod = 'paste' | 'type';

export const DEFAULT_ASSISTANT_NAME = 'Крео';

export interface AppSettings {
    assistantName: string;
    textInputMethod: TextInputMethod;
    historyRetentionDays: number;
}

export const DEFAULT_SETTINGS: AppSettings = {
    assistantName: DEFAULT_ASSISTANT_NAME,
    textInputMethod: 'paste',
    historyRetentionDays: 30,
};
