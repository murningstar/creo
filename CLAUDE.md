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

> **Details:** [`.claude/docs/audio-pipeline.md`](.claude/docs/audio-pipeline.md) — pipeline diagram, models table, GPU compatibility, auto-configuration.

Краткая схема: Микрофон (cpal) → Silero VAD (ort/ONNX) → speech buffer → whisper-rs → fuzzy wake word match / dictation text → Tauri events → Vue frontend. Три потока: capture, VAD processing, whisper transcription.

---

## Platform-Specific Considerations

> **Details:** [`.claude/docs/platform.md`](.claude/docs/platform.md) — Windows (UIPI, Cyrillic paths, NSIS), Linux (Wayland/X11), macOS (future).

---

## UX Requirements

> **Details:** [`.claude/docs/ux-requirements.md`](.claude/docs/ux-requirements.md) — visual feedback, overlay indicator, banners, text input, history.

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

### Rust Backend (src-tauri/)

**Tauri events (Rust → Frontend):**

- Именование: `audio-state-changed`, `vad-state`, `transcription`, `wake-command`, `audio-error`
- Ошибки аудио-пайплайна — событие `audio-error` (НЕ generic `error`)

**PipelineHandle (managed state):**

- Поля приватные, доступ только через методы
- `transition_mode(app, new_mode)` — единственный способ менять mode (атомарно: set + emit). Прямая запись в mode запрещена — гарантирует sync между Rust и frontend
- `join_threads()`, `push_thread()` — safe mutex access (без unwrap, через map_err)

**Domain types:**

- Все payload/model структуры (`AudioMode`, `ModelInfo`, `ModelStatus`, `WakeCommand`, etc.) живут в `audio/mod.rs`
- `commands.rs` — только Tauri command handlers + platform-specific logic (`get_models_dir`)

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
- Tauri конфиги: `tauri.conf.json5` (base), `tauri.windows.conf.json5`, `tauri.linux.conf.json5`. **Platform конфиги — delta-only** (содержат только overrides, Tauri 2 deep-мержит поверх base)
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

**При реализации фичи — обновить roadmap в README.md** (статус `done`, добавить детали). При добавлении новой фичи — добавить строку в соответствующую секцию.
