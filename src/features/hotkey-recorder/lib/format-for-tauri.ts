import type { KeyCombo } from '~/shared/ui/keystroke-recorder/model/types';

function codeToTauriKey(code: string): string {
    if (code.startsWith('Key')) return code.slice(3); // KeyA → A
    if (code.startsWith('Digit')) return code.slice(5); // Digit1 → 1
    if (code.startsWith('Numpad')) return `Num${code.slice(6)}`; // Numpad0 → Num0
    return code; // Backquote, F1, Space, ScrollLock, etc.
}

export function formatForTauri(combo: KeyCombo | null): string {
    if (!combo) return 'Control+Backquote';
    const parts: string[] = [];
    if (combo.ctrl) parts.push('Control');
    if (combo.alt) parts.push('Alt');
    if (combo.shift) parts.push('Shift');
    if (combo.meta) parts.push('Super');
    parts.push(codeToTauriKey(combo.code));
    return parts.join('+');
}
