# Hotkey Limitations — Windows

## API: RegisterHotKey (used by tauri-plugin-global-shortcut)

- Supports any VK code + modifier mask (Ctrl, Alt, Shift, Win)
- Single keys without modifier: supported (pass modifier=0)
- Modifier-only combos (double-tap Ctrl): NOT supported — requires low-level hooks
- First-come-first-served: if another app registered the same combo, fails with ERROR_HOTKEY_ALREADY_REGISTERED

## Reserved by Windows (cannot override)

| Shortcut       | Function                                                 |
| -------------- | -------------------------------------------------------- |
| Win+L          | Lock screen (kernel-level)                               |
| Ctrl+Alt+Del   | Secure Attention Sequence                                |
| F12            | Reserved for debugger                                    |
| Win+key combos | OS docs say "reserved for OS", but some work in practice |

## Specific keys

| Key         | Available              | Notes                                                          |
| ----------- | ---------------------- | -------------------------------------------------------------- |
| ScrollLock  | Yes                    | No conflicts, but missing on most laptops                      |
| F13-F24     | Yes (VK_F13-VK_F24)    | No physical key on standard keyboards. F23 used by Copilot key |
| Pause/Break | Yes                    | Win+Pause = System Properties. Pause alone is free             |
| CapsLock    | Not via RegisterHotKey | Requires low-level hook to suppress toggle                     |
| Media keys  | Yes                    | High conflict with media players                               |

## Known global hotkeys by popular apps

| App                    | Hotkey                                         |
| ---------------------- | ---------------------------------------------- |
| PowerToys Run          | Alt+Space                                      |
| PowerToys Color Picker | Win+Shift+C                                    |
| ShareX                 | Ctrl+PrintScreen, PrintScreen, Alt+PrintScreen |
| Windows Voice Typing   | Win+H                                          |
| Game Bar               | Win+G                                          |
| Clipboard History      | Win+V                                          |
| Snipping Tool          | Win+Shift+S                                    |
| Emoji Panel            | Win+.                                          |
| Task Manager           | Ctrl+Shift+Esc                                 |

## Combo safety analysis

| Combo            | Safe? | Notes                                   |
| ---------------- | ----- | --------------------------------------- |
| Ctrl+Shift+Space | YES   | No known global conflicts               |
| Ctrl+Shift+X     | YES   | VS Code uses in-app only                |
| Ctrl+`           | YES   | No known global conflicts               |
| Ctrl+Alt+Space   | NO    | AltGr issue on international keyboards  |
| Alt+Space        | NO    | Windows system menu (move/resize/close) |
| Alt+`            | OK    | No system conflict on Windows           |

## Ctrl+Shift conflict with Russian layout switching

Ctrl+Shift is commonly used for RU/EN keyboard layout switching on Windows.
Any Ctrl+Shift+key combo may conflict for users with this setting.
Ctrl+` (without Shift) avoids this entirely.

## Low-level hooks (SetWindowsHookEx WH_KEYBOARD_LL)

- Can capture ALL keys including modifier-only combos
- Works across elevated windows (unlike SendInput)
- Performance: every keystroke routes through hook, adds latency
- Timeout: 1000ms max on Win10 1709+, hook silently removed if exceeded
- Can detect press/release for hold-to-talk
