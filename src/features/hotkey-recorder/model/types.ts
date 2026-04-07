import type { KeyCombo } from '~/shared/ui/keystroke-recorder';

export interface HotkeyIssue {
    severity: 'error' | 'warning';
    message: string;
}

export type ComboChecker = (combo: KeyCombo) => HotkeyIssue | null;
