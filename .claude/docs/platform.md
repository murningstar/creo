# Platform-Specific Considerations

## Windows

- **UIPI:** non-elevated процесс не может вводить текст в elevated окна → рекомендуем запуск от admin, баннер если нет
- **Кириллица в путях:** `C:\Users\я\` ломает fopen() в C/C++ библиотеках → модели/кэш всегда в ASCII-путях (`C:\creo-data\`)
- **Установщик (NSIS):** предупреждение о non-ASCII путях, рекомендация "Для всех пользователей" (Program Files)
- **Text injection:** SendInput + KEYEVENTF_UNICODE (enigo)

## Linux

- **Wayland:** ввод текста через enigo экспериментален → fallback на clipboard+paste
- **X11:** XTest работает без проблем
- **Tray:** зависит от DE, не все Wayland DE поддерживают
- **Пути:** UTF-8 нативно, проблем нет

## macOS (будущее)

- **Accessibility permission** обязателен для input injection
- **Microphone permission** обязателен
- **Notarization** для распространения
- **Metal** для GPU ускорения
