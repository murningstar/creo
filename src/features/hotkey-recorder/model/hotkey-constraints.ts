import type { KeyCombo } from '~/shared/ui/keystroke-recorder/model/types';

// Platform identifier matching @tauri-apps/plugin-os Platform type
type Platform = 'windows' | 'linux' | 'macos';

export interface HotkeyIssue {
    severity: 'error' | 'warning';
    message: string;
}

type ComboChecker = (combo: KeyCombo) => HotkeyIssue | null;

// --- Individual constraint checkers ---

// Standalone keys that are safe as single-key hotkeys (no modifier needed)
const SAFE_STANDALONE_KEYS = new Set([
    'ScrollLock',
    'Pause',
    'F13',
    'F14',
    'F15',
    'F16',
    'F17',
    'F18',
    'F19',
    'F20',
    'F21',
    'F22',
    'F23',
    'F24',
]);

const noModifier: ComboChecker = combo => {
    if (!combo.ctrl && !combo.alt && !combo.shift && !combo.meta) {
        if (SAFE_STANDALONE_KEYS.has(combo.code)) return null;
        return {
            severity: 'error',
            message:
                'A modifier key (Ctrl, Alt, Shift, or Super) is required — unless using a dedicated key like Scroll Lock or Pause.',
        };
    }
    return null;
};

// Windows-specific
const winReserved: ComboChecker = combo => {
    // Win+L (lock screen) — kernel-level, cannot override
    if (combo.meta && combo.key === 'l') {
        return { severity: 'error', message: 'Win+L is reserved by Windows (lock screen) and cannot be overridden.' };
    }
    // Ctrl+Alt+Delete — Secure Attention Sequence
    if (combo.ctrl && combo.alt && combo.code === 'Delete') {
        return { severity: 'error', message: 'Ctrl+Alt+Delete is a system reserved sequence on Windows.' };
    }
    // F12 — debugger
    if (combo.code === 'F12' && !combo.ctrl && !combo.alt && !combo.shift && !combo.meta) {
        return { severity: 'warning', message: 'F12 is reserved for the debugger on Windows.' };
    }
    return null;
};

const winConflicts: ComboChecker = combo => {
    // Alt+Space — system window menu
    if (combo.alt && !combo.ctrl && !combo.meta && combo.code === 'Space') {
        return { severity: 'error', message: 'Alt+Space opens the system window menu on Windows.' };
    }
    // Win+H — Windows Voice Typing
    if (combo.meta && combo.key === 'h') {
        return { severity: 'warning', message: 'Win+H activates Windows Voice Typing.' };
    }
    // Win+V — Clipboard History
    if (combo.meta && combo.key === 'v') {
        return { severity: 'warning', message: 'Win+V opens Clipboard History on Windows.' };
    }
    // Ctrl+Shift+Esc — Task Manager
    if (combo.ctrl && combo.shift && combo.code === 'Escape') {
        return { severity: 'error', message: 'Ctrl+Shift+Esc opens Task Manager on Windows.' };
    }
    return null;
};

const winCtrlShiftLayout: ComboChecker = combo => {
    // Ctrl+Shift is commonly used for RU/EN keyboard layout switching
    if (combo.ctrl && combo.shift && !combo.alt && !combo.meta) {
        return {
            severity: 'warning',
            message:
                'Ctrl+Shift is commonly used for keyboard layout switching (RU/EN). This combo may conflict if that setting is enabled.',
        };
    }
    return null;
};

// macOS-specific
const macReserved: ComboChecker = combo => {
    // Cmd+Tab — app switcher
    if (combo.meta && combo.code === 'Tab') {
        return { severity: 'error', message: 'Cmd+Tab is the app switcher on macOS and cannot be overridden.' };
    }
    // Cmd+Space — Spotlight
    if (combo.meta && !combo.ctrl && combo.code === 'Space') {
        return {
            severity: 'warning',
            message: 'Cmd+Space is Spotlight/Raycast on macOS. It can be reassigned, but may cause conflicts.',
        };
    }
    // Ctrl+Cmd+Q — lock screen
    if (combo.ctrl && combo.meta && combo.key === 'q') {
        return { severity: 'error', message: 'Ctrl+Cmd+Q locks the screen on macOS.' };
    }
    return null;
};

const macSequoiaModifier: ComboChecker = combo => {
    // macOS 15 Sequoia requires Cmd or Ctrl modifier for RegisterEventHotKey
    // Exception: F13-F15 (PC ScrollLock/Pause/PrintScreen mapped) work via CGEventTap
    if (!combo.ctrl && !combo.meta && !SAFE_STANDALONE_KEYS.has(combo.code)) {
        return {
            severity: 'error',
            message: 'macOS 15+ requires Cmd or Ctrl modifier for global hotkeys.',
        };
    }
    return null;
};

const macOptionGrave: ComboChecker = combo => {
    // Option+` inserts grave accent character in text fields
    if (combo.alt && combo.code === 'Backquote' && !combo.ctrl && !combo.meta) {
        return { severity: 'error', message: 'Option+` inserts a grave accent character on macOS.' };
    }
    return null;
};

// Linux-specific
const linuxAltGrave: ComboChecker = combo => {
    // Alt+` — GNOME same-app window switching
    if (combo.alt && !combo.ctrl && !combo.meta && combo.code === 'Backquote') {
        return {
            severity: 'warning',
            message: 'Alt+` is used by GNOME for same-app window switching.',
        };
    }
    return null;
};

const linuxSuperConflict: ComboChecker = combo => {
    if (combo.meta) {
        return {
            severity: 'warning',
            message: 'Super key combos conflict with most Linux desktop environments (GNOME, KDE, tiling WMs).',
        };
    }
    return null;
};

const linuxCtrlShiftLayout: ComboChecker = combo => {
    // Same issue as Windows — Ctrl+Shift for RU/EN layout switching
    if (combo.ctrl && combo.shift && !combo.alt && !combo.meta) {
        return {
            severity: 'warning',
            message:
                'Ctrl+Shift is commonly used for keyboard layout switching on Linux (IBus/xkb). This combo may be intercepted before reaching the app.',
        };
    }
    return null;
};

const linuxCtrlSpace: ComboChecker = combo => {
    if (combo.ctrl && !combo.alt && !combo.shift && !combo.meta && combo.code === 'Space') {
        return { severity: 'warning', message: 'Ctrl+Space is commonly used for input source switching on Linux.' };
    }
    return null;
};

// --- Platform constraint registry ---

const PLATFORM_CHECKERS: Record<Platform, ComboChecker[]> = {
    windows: [noModifier, winReserved, winConflicts, winCtrlShiftLayout],
    macos: [noModifier, macReserved, macSequoiaModifier, macOptionGrave],
    linux: [noModifier, linuxAltGrave, linuxSuperConflict, linuxCtrlShiftLayout, linuxCtrlSpace],
};

/**
 * Validate a key combination against platform-specific constraints.
 * Returns all issues found (errors and warnings).
 */
export function validateHotkey(combo: KeyCombo, platform: Platform | null): HotkeyIssue[] {
    if (!platform) return [noModifier(combo)].filter((v): v is HotkeyIssue => v !== null);

    const checkers = PLATFORM_CHECKERS[platform] ?? [noModifier];
    return checkers.map(check => check(combo)).filter((v): v is HotkeyIssue => v !== null);
}
