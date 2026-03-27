# Hotkey Limitations — Linux X11

## API: XGrabKey (used by tauri-plugin-global-shortcut)

- Can grab any keycode + modifier mask on root window
- Single keys without modifier: supported (modifier mask = 0)
- First-come-first-served: BadAccess error if another app already grabbed
- Cannot grab modifier-only keys (Super alone, Ctrl alone)

## The 8-combination problem

X11 treats CapsLock, NumLock, ScrollLock as modifiers.
XGrabKey matches EXACT modifier state.
Must register 4-8 combinations per hotkey to cover all lock states:

- Base
- Base + NumLock (Mod2)
- Base + CapsLock (Lock)
- Base + NumLock + CapsLock

tauri-plugin-global-shortcut handles 4 combinations (no ScrollLock/Mod3).

## Desktop environment reserved shortcuts

### GNOME Shell

| Shortcut        | Function                                  |
| --------------- | ----------------------------------------- |
| Super (tap)     | Activities overview                       |
| Super+Tab       | App switcher                              |
| Super+A         | Show applications                         |
| Super+L         | Lock screen                               |
| Super+Space     | Switch input source                       |
| Alt+Tab         | Window switcher                           |
| Alt+F2          | Run dialog                                |
| Alt+`           | Switch windows of same app — **CONFLICT** |
| Ctrl+Alt+Delete | Power off dialog                          |
| PrintScreen     | Screenshot                                |

### KDE Plasma 6

| Shortcut        | Function                |
| --------------- | ----------------------- |
| Meta (tap)      | App launcher            |
| Meta+Tab        | Switch activity         |
| Meta+Arrow keys | Window tiling           |
| Ctrl+Alt+L      | Lock screen             |
| Ctrl+F1-F4      | Switch virtual desktops |

### XFCE

| Shortcut            | Function                                     |
| ------------------- | -------------------------------------------- |
| Alt+F1-F3           | App finder / context menu                    |
| F6-F12              | Window management (minimize, maximize, etc.) |
| Ctrl+Alt+Left/Right | Switch workspaces                            |

### Tiling WMs (i3/sway)

- $mod+almost-everything (where $mod = Super or Alt)
- Bare F-keys NOT grabbed by default
- User-configurable, cannot predict $mod choice

## Specific keys

| Key                      | Available             | Notes                                        |
| ------------------------ | --------------------- | -------------------------------------------- |
| ScrollLock               | Yes (as key target)   | Mod3 modifier issues; missing on laptops     |
| F13-F24                  | Yes (XKeysym defined) | No physical key on standard keyboards        |
| Super/Meta               | Only in combos        | Modifier, not standalone. Conflicts with DEs |
| Media keys (XF86Audio\*) | Yes, grabbable        | Conflict with gnome-settings-daemon          |
| Pause                    | Yes                   | Historical key-up event issues               |

## CRITICAL: Ctrl+Shift as layout switch

Russian/English keyboard layout switching is commonly configured as Ctrl+Shift.
When active, ALL Ctrl+Shift+letter combos become unreliable:
IBus/xkb intercepts Ctrl+Shift before the letter key registers.

**Any Ctrl+Shift+X combo is unsafe for RU keyboard users.**
Use combos without Shift (Ctrl+`) to avoid this.

## Combo safety analysis

| Combo               | Safe? | Notes                                          |
| ------------------- | ----- | ---------------------------------------------- |
| Ctrl+`              | YES   | No DE conflicts, no layout switch conflict     |
| Ctrl+Shift+X        | RISKY | Ctrl+Shift layout switching conflict           |
| Ctrl+Shift+Space    | RISKY | Same layout switching issue                    |
| Ctrl+\\ (backslash) | YES   | No known conflicts                             |
| Alt+`               | NO    | GNOME uses Alt+` for same-app window switching |
| Super+anything      | RISKY | All DEs use Super heavily                      |

## tauri-plugin-global-shortcut on X11

- Uses global-hotkey crate → XGrabKey
- 4-combination modifier handling (NumLock + CapsLock, not ScrollLock)
- 50ms polling interval for events
- Bug: NumLock mapped to F1 keysym (avoid using NumLock as hotkey)
- F13-F24 correctly mapped
- Media keys correctly mapped
