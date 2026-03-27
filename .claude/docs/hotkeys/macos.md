# Hotkey Limitations — macOS

## API: RegisterEventHotKey (used by tauri-plugin-global-shortcut)

- Carbon Events API, older but functional
- Requires at least one modifier (Cmd or Ctrl) on macOS 15 Sequoia+
- Option-only and Shift-only combos BROKEN on Sequoia (Apple security change)
- Cannot register single keys without modifier
- Cannot capture Fn/Globe key, media keys

## Alternative API: CGEventTap

- Can capture any key including single keys, media keys
- Requires Accessibility permission (kCGEventTapOptionDefault) or Input Monitoring (kCGEventTapOptionListenOnly)
- App must be RESTARTED after user grants permission
- Cannot capture during Secure Keyboard Entry (password fields, 1Password fill)
- NOT used by tauri-plugin-global-shortcut

## Reserved by macOS (cannot override reliably)

| Shortcut            | Function                                          |
| ------------------- | ------------------------------------------------- |
| Cmd+Tab             | App Switcher                                      |
| Cmd+Space           | Spotlight (reassignable but system gets priority) |
| Ctrl+Cmd+Q          | Lock screen                                       |
| Power button combos | Sleep, shutdown, restart                          |

## Reassignable but default-bound

| Shortcut          | Function              |
| ----------------- | --------------------- |
| Ctrl+Up/Down      | Mission Control       |
| Ctrl+Left/Right   | Switch desktop        |
| Ctrl+Space        | Previous input source |
| Ctrl+Option+Space | Next input source     |
| Ctrl+Cmd+Space    | Emoji & Symbols       |
| Double-tap Fn     | Start dictation       |

## Specific keys

| Key        | Available                                                   | Notes                                               |
| ---------- | ----------------------------------------------------------- | --------------------------------------------------- |
| Fn/Globe   | NOT capturable                                              | Hardware-level, only Karabiner can access           |
| F13-F15    | Yes (PC keyboard PrintScreen/ScrollLock/Pause map to these) | Not on MacBook built-in keyboard                    |
| F16-F19    | Yes on old Apple Extended Keyboard                          | Not on modern keyboards                             |
| CapsLock   | Special                                                     | 250ms hardware debounce, can be removed via hidutil |
| Media keys | Via CGEventTap only                                         | Not via RegisterEventHotKey                         |

## macOS 15 Sequoia BREAKING CHANGE

RegisterEventHotKey no longer fires for Option-only or Shift-only modifiers.
Only Cmd+key and Ctrl+key (and combinations with other modifiers) work.
This does NOT affect CGEventTap.

## Known global hotkeys by popular apps

| App              | Hotkey                         |
| ---------------- | ------------------------------ |
| Raycast          | Cmd+Space (replaces Spotlight) |
| Alfred           | Cmd+Space or Option+Space      |
| SuperWhisper     | Option+Space                   |
| Wispr Flow       | Fn+Space                       |
| Rectangle/Magnet | Ctrl+Option+Arrow keys         |

## Combo safety analysis

| Combo            | Safe?         | Notes                                                       |
| ---------------- | ------------- | ----------------------------------------------------------- |
| Ctrl+Shift+X     | YES           | No system or app conflicts                                  |
| Cmd+Shift+X      | MODERATE      | Conflicts with strikethrough in Word/Google Docs in-app     |
| Ctrl+Shift+Space | MODERATE      | Close to Ctrl+Space (input source switch)                   |
| Option+Space     | NO on Sequoia | Broken with RegisterEventHotKey; inserts non-breaking space |
| Ctrl+`           | YES           | No known conflicts                                          |
| Alt+`            | NO            | Option+` inserts grave accent in text fields                |

## Permission prompts

- Accessibility: one-time grant, persists across reboots
- Input Monitoring: one-time grant, persists
- macOS 15+: monthly re-auth for Screen Recording (NOT for Accessibility/Input Monitoring)
- User must restart app after granting permission

## Pre-permission banner needed

Before requesting Accessibility permission, show user-facing explanation:
"Creo needs keyboard access to detect hotkeys from any app and to type dictated text."
