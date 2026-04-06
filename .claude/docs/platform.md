# Platform-Specific Considerations

## Windows

- **UIPI:** non-elevated процесс не может вводить текст в elevated окна → рекомендуем запуск от admin, баннер если нет
- **Кириллица в путях:** `C:\Users\я\` ломает fopen() в C/C++ библиотеках → модели/кэш всегда в ASCII-путях (`C:\creo-data\`)
- **Установщик (NSIS):** предупреждение о non-ASCII путях, рекомендация "Для всех пользователей" (Program Files)
- **Text injection:** SendInput + KEYEVENTF_UNICODE (enigo)

## Linux

- **Paste split (X11 vs Wayland):** paste.rs определяет display server через env vars
    - **X11:** `Ctrl+V` (стандартный GUI paste, XTest)
    - **Wayland:** `Ctrl+Shift+V` (работает в терминалах и большинстве GUI). Лог-warning (один раз) о потенциальной проблеме с раскладкой
    - **Проблема с раскладкой (Wayland):** при не-English раскладке Ctrl+Shift+V может не распознаваться. Workaround (не реализован): D-Bus `org.kde.keyboard /Layouts setLayout` для переключения перед paste
- **X11:** XTest работает без проблем
- **Tray:** зависит от DE, не все Wayland DE поддерживают
- **Пути:** UTF-8 нативно, проблем нет
- **GCC 15 / Fedora 43:** whisper.cpp (и whisper-rs) крашится с `std::bad_alloc` в `std::regex` при компиляции GPT-2 токенизатора ([GCC Bug 86164](https://gcc.gnu.org/bugzilla/show_bug.cgi?id=86164)). Рекурсивный DFS в libstdc++ `std::regex`. Исправлено в GCC 16 (Fedora 44). build.rs детектирует GCC 15 и выдаёт `cargo:warning`. Workaround: `CC=clang CXX=clang++` или `.cargo/config.toml` (см. README)
- **OpenVINO на Intel CPU (Alder Lake):** энкодер whisper загружается и работает, но whisper.cpp крашится в токенизаторе (тот же GCC 15 баг). При фиксе GCC или переходе на Clang — OpenVINO даст ускорение энкодера
- **Overlay на Wayland:** `alwaysOnTop` и `ignoreCursorEvents` имеют ограничения. lib.rs логирует ошибки вместо `let _ =`, эмитит `overlay-capability-degraded` event при failure. На Wayland выводит warning в лог. Ref: [Tauri Window Customization](https://v2.tauri.app/learn/window-customization/)

## macOS (будущее)

- **Accessibility permission** обязателен для input injection
- **Microphone permission** обязателен
- **Notarization** для распространения
- **Metal** для GPU ускорения
