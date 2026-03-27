# Hotkey Cross-Platform Summary

## Implementation per platform

| Platform      | API                         | Crate                        | Hold-to-talk | Extra permissions                        |
| ------------- | --------------------------- | ---------------------------- | ------------ | ---------------------------------------- |
| Windows       | RegisterHotKey              | tauri-plugin-global-shortcut | Yes          | None                                     |
| macOS         | RegisterEventHotKey         | tauri-plugin-global-shortcut | Yes          | Accessibility permission                 |
| Linux X11     | XGrabKey                    | tauri-plugin-global-shortcut | Yes          | None                                     |
| Linux Wayland | XDG Portal → evdev fallback | ashpd → evdev crate          | Yes          | Portal: none. evdev: input group + relog |

## Universal safe combos (work on ALL platforms without conflicts)

| Combo            | Windows | macOS    | Linux X11             | Linux Wayland | Verdict                 |
| ---------------- | ------- | -------- | --------------------- | ------------- | ----------------------- |
| **Ctrl+`**       | Safe    | Safe     | Safe                  | Safe          | **RECOMMENDED DEFAULT** |
| Ctrl+\\          | Safe    | Safe     | Safe                  | Safe          | Good alternative        |
| Ctrl+Shift+.     | Safe    | Safe     | RISKY (layout switch) | RISKY         | Not for RU users        |
| Ctrl+Shift+X     | Safe    | Safe     | RISKY (layout switch) | RISKY         | Not for RU users        |
| Ctrl+Shift+Space | Safe    | Moderate | RISKY (layout switch) | RISKY         | Not for RU users        |

## Combos with platform-specific conflicts

| Combo          | Problem platform | Conflict                                                    |
| -------------- | ---------------- | ----------------------------------------------------------- |
| Alt+`          | GNOME Linux      | Same-app window switching                                   |
| Alt+Space      | Windows          | System window menu                                          |
| Option+Space   | macOS Sequoia+   | Broken with RegisterEventHotKey; inserts non-breaking space |
| Ctrl+Space     | macOS/Linux      | Input source switching                                      |
| Super+anything | Linux            | DE launcher, tiling WM bindings                             |
| F12            | Windows          | Reserved for debugger                                       |
| Media keys     | All              | Conflict with media players                                 |
| Bare backtick  | All              | Cannot type ` in any app                                    |

## Keys NOT available on laptops

| Key         | Desktop keyboard | Laptop                        |
| ----------- | ---------------- | ----------------------------- |
| ScrollLock  | Usually          | Rarely (Fn+something, varies) |
| Pause/Break | Usually          | Rarely                        |
| F13-F24     | Never (standard) | Never                         |
| Numpad keys | Usually          | Rarely                        |

## The Ctrl+Shift problem for Russian users

Ctrl+Shift is the most common RU/EN keyboard layout switch on Windows and Linux.
When active, pressing Ctrl+Shift triggers layout switch BEFORE the third key registers.
ALL Ctrl+Shift+key combos become unreliable.

**Solution:** Use combos without Shift modifier (Ctrl+`), or document that users
should switch layout switching to Alt+Shift or Win+Space.

## macOS Sequoia (15+) restriction

RegisterEventHotKey no longer fires for Option-only or Shift-only modifiers.
Must include Cmd or Ctrl. Ctrl+` is safe.

## Wayland special considerations

- No global hotkey API in Wayland protocol itself
- XDG Portal: app proposes, compositor decides (not guaranteed exact binding)
- evdev: works universally but requires input group + relog
- Banner needed for Wayland users about input group requirement

## Decision: Creo default hotkey

**Ctrl+`** (backtick)

Reasoning:

1. No conflict on any platform (Windows, macOS, Linux X11, Linux Wayland)
2. No Shift modifier → no RU layout switching conflict
3. Available on all keyboards including laptops
4. One hand (left Ctrl + backtick key above Tab)
5. OpenWhispr uses backtick as default (proven UX pattern)
6. Works with RegisterHotKey, RegisterEventHotKey, XGrabKey, evdev
7. Works on macOS Sequoia+ (has Ctrl modifier)
8. Not used globally by any known app
