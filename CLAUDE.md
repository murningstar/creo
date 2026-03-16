# Creo — Claude Code Context

## Project Overview

Десктопный голосовой помощник (Windows + Linux, macOS позже) на Nuxt 3 + Tauri 2 с Feature-Sliced Design архитектурой. Полностью self-hosted, все ML модели работают локально.

**Статус:** MVP аудио-пайплайн реализован (cpal → VAD → whisper-rs). Диктовка через whisper-rs как placeholder до интеграции ct2rs/parakeet-rs.

---

## Concept

**Creo** — always-listening голосовой ассистент для десктопа. Активируется голосовыми командами (wake words), работает полностью offline.

### Wake commands (русский язык)

| Команда              | Действие                                               | Завершение                |
| -------------------- | ------------------------------------------------------ | ------------------------- |
| **"Крео, приём"**    | Command mode — MVP: активация Kando (wheel menu)       | Автоматически             |
| **"Крео, вписывай"** | Dictation mode — непрерывная диктовка в активный input | По команде "Крео, готово" |
| **"Крео, готово"**   | Завершение диктовки                                    | —                         |

### Функциональные возможности

1. **Wake word detection** — активация голосом через Silero VAD + whisper-rs tiny
2. **Dictation** — диктовка текста с вводом через enigo (SendInput / clipboard+paste)
3. **Voice commands** — голосовые команды (MVP: запуск Kando)
4. **Auto-configuration** — автоопределение железа, подбор оптимальной модели
5. **History** — история команд/диктовок с настраиваемым retention
6. **Hotkey fallback** — горячая клавиша как альтернатива wake word

---

## Tech Stack

### Frontend

- Nuxt 3 (Vue 3 Composition API, `<script setup>`)
- TypeScript (strict mode)
- Tauri 2 (native desktop: Windows, Linux)
- Pinia (state management)
- NuxtUI (UI components, prefix `u-`)
- TailwindCSS
- VueUse (утилиты)
- pnpm (package manager, НИКОГДА npm)

### Rust Backend (src-tauri/)

- **cpal** — захват микрофона
- **Silero VAD** (ONNX Runtime / `ort`) — always-on voice activity detection (~0.4% CPU)
- **whisper-rs** (whisper.cpp, GGML tiny model) — wake word detection only
- **Main STT — два движка, user-selectable:**
    - **CTranslate2 via ct2rs** — для NVIDIA GPU + CPU (скорость faster-whisper, CTranslate2 модели)
    - **Parakeet TDT 0.6B via parakeet-rs** — для AMD/Intel GPU (DirectML/Vulkan) + CPU (ONNX модель ~600MB, лучший WER для русского)
- **enigo** — ввод текста в активное приложение (гибрид: SendInput < 100 символов, clipboard+paste для длинного)
- **rodio/cpal** — звуковой feedback

---

## Audio Pipeline Architecture

```
Микрофон (cpal, always-on)
    │
    ▼
Silero VAD (~1.8MB ONNX, always-on, <1% CPU)
    │ речь обнаружена
    ▼
Буфер 2-3 сек аудио
    │
    ▼
whisper-rs tiny (~75MB GGML, CPU burst ~0.2s)
    │ fuzzy match "Крео, приём/вписывай/готово"
    │
    ├─ "Крео, приём" → Tauri IPC → frontend: command mode
    │   → Запуск Kando (MVP)
    │
    ├─ "Крео, вписывай" → Tauri IPC → frontend: dictation mode
    │   → Прогрев основного STT (ct2rs или parakeet-rs)
    │   → Непрерывная транскрипция → enigo → ввод текста
    │
    ├─ "Крео, готово" → Tauri IPC → frontend: stop dictation
    │   → Остановка STT, финализация текста
    │
    └─ Нет совпадения → discard, возврат к VAD listening
```

### Модели (скачиваются при первом запуске)

| Модель                       | Назначение               | Размер      | Формат      |
| ---------------------------- | ------------------------ | ----------- | ----------- |
| Silero VAD                   | Voice Activity Detection | ~1.8 MB     | ONNX        |
| Whisper tiny                 | Wake word detection      | ~75 MB      | GGML        |
| Whisper small/large-v3-turbo | Main STT (CTranslate2)   | 500MB-1.5GB | CTranslate2 |
| Parakeet TDT 0.6B            | Main STT (альтернатива)  | ~600 MB     | ONNX        |

### GPU Compatibility

| GPU                    | CTranslate2 (ct2rs) | whisper.cpp (whisper-rs) | Parakeet (ONNX Runtime) |
| ---------------------- | ------------------- | ------------------------ | ----------------------- |
| NVIDIA (CUDA)          | ✓                   | ✓                        | ✓                       |
| AMD (Windows/DirectML) | ✗                   | ✓ (Vulkan)               | ✓                       |
| AMD (Linux/ROCm)       | ✗                   | ✓ (Vulkan)               | ✓                       |
| Intel iGPU             | ✗                   | ✓ (Vulkan)               | ✓ (DirectML/OpenVINO)   |
| CPU only               | ✓ (int8)            | ✓                        | ✓                       |

### Auto-Configuration (первый запуск)

1. Определяем GPU (vendor, VRAM), CPU, RAM
2. Подбираем оптимальный движок + модель + квантизацию
3. Показываем пользователю в понятном виде (без технических деталей)
4. Пользователь может переопределить выбор

---

## Platform-Specific Considerations

### Windows

- **UIPI:** non-elevated процесс не может вводить текст в elevated окна → рекомендуем запуск от admin, баннер если нет
- **Кириллица в путях:** `C:\Users\я\` ломает fopen() в C/C++ библиотеках → модели/кэш всегда в ASCII-путях (`C:\creo-data\`)
- **Установщик (NSIS):** предупреждение о non-ASCII путях, рекомендация "Для всех пользователей" (Program Files)
- **Text injection:** SendInput + KEYEVENTF_UNICODE (enigo)

### Linux

- **Wayland:** ввод текста через enigo экспериментален → fallback на clipboard+paste
- **X11:** XTest работает без проблем
- **Tray:** зависит от DE, не все Wayland DE поддерживают
- **Пути:** UTF-8 нативно, проблем нет

### macOS (будущее)

- **Accessibility permission** обязателен для input injection
- **Microphone permission** обязателен
- **Notarization** для распространения
- **Metal** для GPU ускорения

---

## UX Requirements

### Visual Feedback

- **Начало записи:** пульс-волна (circle) расширяющаяся от индикатора на весь экран — привлекает периферийное зрение
- **Во время диктовки:** компактный circular waveform индикатор (не широкий прямоугольник)
- **Idle:** еле заметный индикатор (как в OpenWhispr)

### Overlay Indicator (отдельное окно)

- Отдельное Tauri окно: AlwaysOnTop, fullscreen, полностью прозрачный фон
- Вся область click-through (прокликивается насквозь к приложениям под ним)
- Пульс/волна при старте записи видна периферийным зрением, не мешает работе
- Tauri window options: `transparent`, `decorations: false`, `always_on_top`, `ignore_cursor_events`

### Banners / Guides

- Admin elevation (Windows) — последствия работы без admin
- Модели — гайд выбора при первом запуске
- Кириллица в путях (Windows) — предупреждение + автофикс
- Wayland limitations (Linux) — оповещение о fallback

### Text Input (enigo)

- < 100 символов: SendInput (не засоряет clipboard)
- ≥ 100 символов: save clipboard → paste → restore clipboard
- Настройка в settings: auto / always type / always paste

### History

- Настраиваемый retention (дни)
- Доступна из настроек и при первом старте

---

## Architecture: Feature-Sliced Design (FSD)

```
src/
├── app/              # Приложение (entrypoint, plugins, styles)
├── pages/            # Страницы (маршруты Nuxt)
├── widgets/          # Составные виджеты
├── features/         # Функциональности (независимые фичи)
├── entities/         # Бизнес-сущности (stores, models)
└── shared/           # Переиспользуемые компоненты и утилиты
```

**Aliases (nuxt.config.ts):**

- `@` → `./src`
- `@app` → `./src/app`
- `@pages` → `./src/pages`
- `@widgets` → `./src/widgets`
- `@features` → `./src/features`
- `@entities` → `./src/entities`
- `@shared` → `./src/shared`

**Экспериментальная фича:** папки `server/` внутри любого FSD сегмента автоматически сканируются Nitro.

---

## Key Configuration (nuxt.config.ts)

- **SSR отключен** (`ssr: false`) — для совместимости с Tauri
- **srcDir:** `./src` (не `/app` из-за коллизии с FSD)
- **App.vue:** `@/app/entrypoint/app.vue`
- **Layouts:** `shared/ui/layouts`
- **Plugins:** `app/plugins`
- **Auto-import компонентов из shared:** паттерн `{shared}/**/ui/*/*.vue`, префикс `c`
- **DevServer:** http://0.0.0.0:4730 (без HTTPS)

---

## Conventions

### Naming

**Компоненты:**

- `u-*` — NuxtUI
- `c-*` — Custom shared компоненты (auto-imported)
- `C*` — Импортируемые компоненты (PascalCase)

**Store методы:**

- `_privateMethod` или `__internalMethod` — приватные/внутренние
- Публичные computed — без префикса: `isListening`, `isNativePlatform`
- `readonly()` для защиты состояния от прямого изменения

**Типы:**

- PascalCase: `AudioMode`, `CurrentNativePlatform`, `WakeCommand`

### File Structure per Segment

```
segment/
├── index.ts           # Public API (re-export)
├── model/             # Типы, константы
│   └── types.ts
├── ui/                # Vue компоненты
├── lib/               # Утилиты
├── infra/             # Stores, API calls
└── server/            # Серверный код (опционально)
```

---

## Entities

**platform** (`entities/platform/`):

- Pinia store: `usePlatformStore()`
- Определяет платформу через `@tauri-apps/plugin-os`
- Computed: `isNativePlatform`, `isNativeDesktop`, `isWebBrowser`

**audio** (`entities/audio/`):

- Pinia store: `useAudioStore()`
- `AudioMode` enum: Idle, Listening, Dictation, Processing
- Computed: `isListening`, `isDictation`, `isProcessing`, `isIdle`

---

## Commands

```bash
# Development
pnpm dev             # Nuxt dev server (http://0.0.0.0:4730)
pnpm tauri:dev       # Tauri + Nuxt dev

# Build
pnpm build           # Nuxt build
pnpm tauri:build     # Tauri production build

# Lint (use --quiet for verification, NOT `pnpm lint` — it has --debug)
pnpm exec eslint --quiet .
pnpm format          # Prettier
```

---

## Important Rules

### UX/UI Protocol

**Все решения по UX/UI должны быть согласованы с пользователем перед реализацией.** Не придумывать и не додумывать: layout, copy, поведение, анимации, баннеры, flow. Если спецификации нет в этом файле — спросить перед реализацией. В roadmap такие фичи отмечены как "requires UX design".

---

## Important Notes

- SSR отключен — приложение работает как SPA
- Tauri конфиги: `tauri.conf.json5`, `tauri.windows.conf.json5`, `tauri.linux.conf.json5`
- Tauri window: 400x600 (компактный формат для voice assistant)
- Auto-import компонентов из shared с префиксом `c-`
- Layouts в `shared/ui/layouts`
- Модели хранятся в ASCII-путях (Windows: `C:\creo-data\`, Linux: `~/.local/share/creo/`)
- Порт dev server: 4730

### Build Dependencies (Rust)

- **LLVM/Clang** — требуется для `whisper-rs-sys` (bindgen). Windows: `winget install LLVM.LLVM`, задать `LIBCLANG_PATH="C:/Program Files/LLVM/bin"`
- **CMake** — требуется для `whisper-rs-sys` (компиляция whisper.cpp). Windows: `winget install Kitware.CMake`, добавить в PATH

---

## Roadmap

Трекинг всех фичей проекта. Статусы: `done`, `in-progress`, `planned`, `requires UX design`.

### MVP (Audio Pipeline) — `done`

| Фича                               | Статус | Детали                                              |
| ---------------------------------- | ------ | --------------------------------------------------- |
| cpal capture + rubato resampling   | done   | 48kHz→16kHz mono f32                                |
| Silero VAD (ort/ONNX)              | done   | 512-sample chunks, threshold 0.5                    |
| whisper-rs transcription           | done   | Используется base модель как placeholder            |
| Wake word fuzzy matching (strsim)  | done   | 3 команды: приём, вписывай, готово                  |
| Pipeline orchestration (3 threads) | done   | Processing + Transcription + Capture                |
| Tauri IPC (events + commands)      | done   | start/stop_listening, get_audio_state, test_capture |
| Frontend state sync                | done   | Pinia store + Tauri event listeners                 |
| Basic pulse indicator              | done   | Анимация при активном состоянии                     |

### Post-MVP — Rust Backend

| Фича                             | Статус  | Зависимости           | Детали                                                                                                        |
| -------------------------------- | ------- | --------------------- | ------------------------------------------------------------------------------------------------------------- |
| ct2rs (CTranslate2)              | planned | —                     | Основной STT для NVIDIA GPU + CPU. Заменит whisper-rs для диктовки. Rust crate: `ct2rs`                       |
| parakeet-rs (Parakeet TDT)       | planned | —                     | Основной STT для AMD/Intel GPU + CPU. ONNX модель ~600MB. Rust crate: `parakeet-rs`                           |
| STT engine trait/abstraction     | planned | ct2rs или parakeet-rs | Общий интерфейс для подмены движка. Текущий `Transcriber` — точка абстракции                                  |
| enigo text injection             | planned | —                     | Гибрид: SendInput <100 символов, clipboard+paste для длинного. **Requires UX design:** настройка режима ввода |
| Sound feedback (rodio/cpal)      | planned | —                     | **Requires UX design:** какие звуки, на какие события (wake word? start/stop dictation?)                      |
| Kando integration                | planned | —                     | **Requires UX design:** механизм запуска (shell command? hotkey? IPC?)                                        |
| Hotkey fallback                  | planned | —                     | **Requires UX design:** какая клавиша, настраиваемость, глобальный хоткей через Tauri                         |
| Model download mechanism         | planned | —                     | **Requires UX design:** UI прогресса скачивания, откуда качать, checksum, retry, оффлайн-fallback             |
| Configurable model paths         | planned | —                     | Каноничные пути: Windows `C:\creo-data\`, Linux `~/.local/share/creo/`. MVP temp: `C:/creo-models/`           |
| Whisper tiny model for wake word | planned | —                     | Сейчас base (~150MB), целевая tiny (~75MB). Переход после стабилизации пайплайна                              |

### Post-MVP — Auto-Configuration

| Фича                        | Статус                 | Детали                                                                                        |
| --------------------------- | ---------------------- | --------------------------------------------------------------------------------------------- |
| Hardware detection          | planned                | GPU vendor/VRAM, CPU, RAM                                                                     |
| Engine/model recommendation | planned                | На основе hardware → оптимальный движок + модель + квантизация                                |
| First-launch wizard         | **requires UX design** | UI: как показать рекомендацию, как пользователь переопределяет выбор. Без технических деталей |

### Post-MVP — UX / Frontend

| Фича                               | Статус                 | Детали                                                                              |
| ---------------------------------- | ---------------------- | ----------------------------------------------------------------------------------- |
| Overlay indicator (отдельное окно) | **requires UX design** | Transparent, always-on-top, click-through. Pulse wave видна периферийным зрением    |
| Circular waveform (диктовка)       | **requires UX design** | Компактный индикатор вместо прямоугольника                                          |
| Subtle idle indicator              | **requires UX design** | Еле заметный, как в OpenWhispr                                                      |
| Settings page                      | **requires UX design** | Scope: STT engine, text input mode, history retention, hotkey, model management     |
| History UI                         | **requires UX design** | Список команд/диктовок с retention в днях. Доступна из settings и при первом старте |

### Post-MVP — Banners / Guides

| Баннер                | Статус                 | Платформа | Детали                                   |
| --------------------- | ---------------------- | --------- | ---------------------------------------- |
| Admin elevation       | **requires UX design** | Windows   | Последствия работы без admin (UIPI)      |
| Model guide           | **requires UX design** | All       | Гайд выбора при первом запуске           |
| Cyrillic path warning | **requires UX design** | Windows   | Предупреждение + автофикс                |
| Wayland limitations   | **requires UX design** | Linux     | Оповещение о fallback на clipboard+paste |

### Future (post-Windows/Linux)

| Фича                       | Платформа | Детали                                                          |
| -------------------------- | --------- | --------------------------------------------------------------- |
| macOS support              | macOS     | Accessibility + Microphone permissions, Notarization, Metal GPU |
| Apple Silicon optimization | macOS     | Metal бэкенд для ONNX Runtime и whisper.cpp                     |
