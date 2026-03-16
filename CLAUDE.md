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

Единый источник правды — **[README.md → Roadmap](README.md#roadmap)**. Все статусы, детали и "requires design" маркеры ведутся там.
