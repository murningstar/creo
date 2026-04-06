# Hotkey Limitations — Linux Wayland

## The fundamental problem

Wayland's security model prevents apps from intercepting global keyboard input.
No equivalent of X11's XGrabKey exists. The compositor owns all input routing.

## Approach 1: XDG GlobalShortcuts Portal (preferred where available)

D-Bus API: `org.freedesktop.portal.GlobalShortcuts`
Rust crate: `ashpd` v0.13.0

### Compositor support (March 2026)

| Compositor            | Status                   | Hold-to-talk (Activated/Deactivated)  |
| --------------------- | ------------------------ | ------------------------------------- |
| KDE Plasma 6.4+       | WORKING                  | Yes                                   |
| GNOME 49+             | WORKING                  | Unreliable (may not fire Deactivated) |
| GNOME 48.5-48.x       | Partial (rebinding bugs) | Unreliable                            |
| Hyprland              | WORKING                  | Yes                                   |
| Sway                  | NOT IMPLEMENTED          | N/A                                   |
| wlroots-based (other) | NOT IMPLEMENTED          | N/A                                   |

### How it works

1. App creates session via D-Bus
2. App proposes shortcuts with descriptions and preferred triggers
3. Compositor shows user dialog to confirm/change bindings
4. Compositor fires Activated/Deactivated signals

### Limitations

- App CANNOT force exact key binding — only propose preferred
- Sway deliberately refuses to implement (waiting for native Wayland protocol)
- GNOME may not fire Deactivated signal reliably (push-to-talk unreliable)
- No permissions needed (user confirms via system dialog)

### global-hotkey PR #162

- Status: OPEN (not merged, March 2026)
- Uses ashpd v0.12.0
- Tested on KDE 6.4-6.5, GNOME 48.5-49
- Track: https://github.com/tauri-apps/global-hotkey/pull/162

## Approach 2: evdev direct (universal fallback)

Read raw keyboard events from /dev/input/event\* at kernel level.
Completely bypasses Wayland — works on ALL compositors.

### Rust crates

- `evdev` crate: direct kernel interface, mature
- `rdev` with `unstable_grab`: grab() DISABLED on Wayland (PR #158). Only listen() works.
- Recommendation: use `evdev` crate directly, not rdev.

### Capabilities

- All keys: F1-F24, media keys, ScrollLock, Pause, any physical key
- Press/release/repeat events (hold-to-talk fully supported)
- Compositor-agnostic (kernel level)

### Requirements

- User must be in `input` group: `sudo usermod -aG input $USER`
- REQUIRES RELOG after adding to group (group membership not active until new login session)
- Flatpak/Snap: NOT accessible without escape hatch
- SELinux/AppArmor may restrict /dev/input access

### Combo hotkey reliability

Комбо-хоткеи (Modifier+Key) ненадёжны при перехвате через evdev: первая клавиша утекает в систему до детекции комбо. Одиночная не-символьная клавиша (F9, ScrollLock) значительно надёжнее и удобнее.

**В коде:** `hotkey-constraints.ts` выдаёт `linuxComboUnreliable` warning для любого modifier combo на Linux. Предлагает single-key альтернативы (ScrollLock, Pause, F13+).

### Security implications

- Reading /dev/input = seeing ALL keyboard input (passwords, etc.)
- Functionally equivalent to keylogger capability
- Users must explicitly trust the app
- Justifiable for voice assistant: already has microphone access (continuous audio)

## Approach 3: Compositor-specific shims

| Compositor | Mechanism                            | Hold-to-talk     |
| ---------- | ------------------------------------ | ---------------- |
| GNOME      | gsettings custom shortcut via D-Bus  | NO (toggle only) |
| KDE        | org.kde.kglobalaccel D-Bus           | Yes              |
| Hyprland   | hyprctl keybindings + D-Bus          | Yes              |
| Sway       | IPC bindsym + shell command callback | Depends          |

Requires per-compositor implementation. OpenWhispr uses this approach.

## Recommended strategy for Creo

1. Try XDG Portal first (ashpd crate, D-Bus introspection to check availability)
2. If portal unavailable (Sway, old GNOME) → fall back to evdev
3. Show banner to user:
    - If evdev needed + not in input group: "For hotkey on Wayland, add yourself to input group and relog"
    - If portal available: seamless, no extra steps

## Combo safety (same as X11 for content)

| Combo        | Safe? | Notes                                    |
| ------------ | ----- | ---------------------------------------- |
| Ctrl+`       | YES   | No compositor conflicts                  |
| Ctrl+Shift+X | RISKY | Ctrl+Shift layout switching for RU users |
| Alt+`        | NO    | GNOME same-app window switching          |

Note: On Wayland, exact key binding depends on compositor behavior.
evdev captures raw scancodes, so any key works at that level.
Portal approach: compositor may remap proposed shortcut.
