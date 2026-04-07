# Creo — Claude Code Context

## Project Overview

Десктопный голосовой помощник (Windows + Linux, macOS позже) на Nuxt 3 + Tauri 2 с Feature-Sliced Design архитектурой. Полностью self-hosted, все ML модели работают локально.

**Статус:** Аудио-пайплайн: cpal → Silero VAD v6 → Google speech-embedding + DTW wake word detection → whisper-rs base (dictation fallback, placeholder до интеграции parakeet-rs). State machine: Off/Standby/Dictation/Processing/AwaitingSubcommand. Mode transitions без restart pipeline.

---

## Concept

**Creo** — always-listening голосовой ассистент для десктопа. Активируется голосовыми командами (wake words), работает полностью offline.

### Wake commands (русский язык)

| Команда              | Действие                                                            | Завершение                 |
| -------------------- | ------------------------------------------------------------------- | -------------------------- |
| **"Крео, приём"**    | AwaitingSubcommand — ожидание подкоманды (DTW / Vosk / LLM cascade) | Таймаут или "Крео, отмена" |
| **"Крео, вписывай"** | Dictation mode — непрерывная диктовка в активный input              | По команде "Крео, готово"  |
| **"Крео, готово"**   | Завершение диктовки                                                 | —                          |
| **"Крео, отмена"**   | Отмена диктовки (без injection текста)                              | —                          |

### Функциональные возможности

1. **Wake word detection** — активация голосом через Silero VAD v6 + Google speech-embedding (96-dim ONNX) + DTW frame-level matching
2. **Dictation** — диктовка текста с вводом через enigo (SendInput / clipboard+paste)
3. **Voice commands** — голосовые подкоманды после "приём" (AwaitingSubcommand mode)
4. **Subcommand cascade** — tiered recognition после "приём" (Tier 1: DTW implemented, Tier 2: Vosk planned, Tier 3: Qwen3+GBNF planned)
5. **Overlay indicator** — transparent always-on-top click-through window showing audio state
6. **System tray** — tray icon с "Show Dashboard" / "Quit", hide-to-tray при закрытии main window
7. **Auto-configuration** — автоопределение железа, подбор оптимальной модели
8. **History** — история команд/диктовок с настраиваемым retention
9. **Hotkey fallback** — горячая клавиша как альтернатива wake word

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
- **Silero VAD v6** (ONNX Runtime / `ort`) — always-on voice activity detection (~0.4% CPU)
- **Google speech-embedding** (mel + embedding ONNX) — 96-dim wake word embeddings, language-agnostic
- **dtw_rs** — DTW frame-level matching для wake word detection
- **parakeet-rs** — основной STT (Parakeet TDT 0.6B, ONNX Runtime: CUDA/DirectML/CPU). Целевой движок для всех платформ
- **whisper-rs** (whisper.cpp, GGML base model) — fallback STT, текущий placeholder до интеграции parakeet-rs
- ct2rs (CTranslate2) — отложен до реализации всех основных фич; актуален для оптимизации пограничных конфигураций (Intel CPU-only). Детали и блокеры в [audio-pipeline.md](.claude/docs/audio-pipeline.md#потенциал-для-будущей-оптимизации-ct2rs)
- **embedding.rs** — shared EmbeddingExtractor (mel+embedding ONNX), DTW utilities, FrameSequence type, `save_frames_file()`/`load_frames_file()`. Используется в wakeword.rs и subcommand.rs
- **subcommand.rs** — SubcommandCascade, SubcommandTier trait, DtwTier, manifest types (SubcommandDef, ParametricTemplate, SlotDef)
- **capture.rs** — AudioCapture (cpal wrapper), AudioResampler, `capture_speech_vad()` (shared VAD capture loop)
- **enigo + arboard** — ввод текста в активное приложение. Два режима: Paste (clipboard + Ctrl+V, default) и Type (enigo char-by-char). Режим выбирается в settings
- **rodio/cpal** — звуковой feedback

---

## Audio Pipeline Architecture

> **Details:** [`.claude/docs/audio-pipeline.md`](.claude/docs/audio-pipeline.md) — pipeline diagram, models table, GPU compatibility, auto-configuration.
> **Evolution plan:** [`.claude/docs/evolution-plan.md`](.claude/docs/evolution-plan.md) — tiered cascade architecture, модели по ролям, эволюционный путь от текущего состояния к целевому. **Сверяться:** при любых изменениях в audio pipeline, при анализе текущего состояния точек соприкосновения технологий, при неполадках (плохое определение wake words, плохая транскрипция, false positives).
> **Architecture audit:** [`.claude/docs/architecture-audit.md`](.claude/docs/architecture-audit.md) — 25 findings от 3 independent auditors (2026-03-27). Статусы обновляются по мере фиксов. **Сверяться:** перед любой cross-cutting доработкой, при архитектурных решениях.

Краткая схема: Микрофон (cpal) → Silero VAD v6 (ort/ONNX) → speech buffer → [wake words: Google embedding + DTW] / [dictation: whisper-rs base] → Tauri events → Vue frontend. Три потока: capture, VAD processing, transcription (DTW + whisper).

**STT engine selection:**

- `resolve_stt_engine(preference)` принимает `"auto"` / `"parakeet"` / `"whisper"` из frontend settings
- `start_listening` и `start_dictation` принимают optional `stt_engine` parameter

> **ВАЖНО:** При изменении pipeline поведения — обновлять CLAUDE.md, audio-pipeline.md и evolution-plan.md **в том же коммите**. Stale docs = stale decisions.

---

## Platform-Specific Considerations

> **Details:** [`.claude/docs/platform.md`](.claude/docs/platform.md) — Windows (UIPI, Cyrillic paths, NSIS), Linux (Wayland/X11), macOS (future).

---

## UX Requirements

> **Details:** [`.claude/docs/ux-requirements.md`](.claude/docs/ux-requirements.md) — visual feedback, overlay indicator, banners, text input, history.

---

## Architecture: Feature-Sliced Design (FSD)

> Документация: https://feature-sliced.design
> Валидация: `/fsd-check` — скилл для проверки FSD-соответствия

```
src/
├── app/              # Wiring: entrypoint, plugins, styles, dictation-flow
├── pages/            # Страницы (маршруты Nuxt), организуют взаимодействие фич
├── widgets/          # Составные блоки: композиция фич для переиспользования между страницами
├── features/         # Пользовательские взаимодействия (переиспользуемые между страницами)
├── entities/         # Бизнес-сущности (stores, models, типы)
└── shared/           # Фундамент: UI kit, утилиты, wire-format types, БЕЗ бизнес-логики
```

### Главное правило: иерархия импортов

**Модуль может импортировать ТОЛЬКО из слоёв СТРОГО НИЖЕ:**

```
app → pages → widgets → features → entities → shared
```

- `shared/` и `app/` — не имеют слайсов, только сегменты. Сегменты внутри одного слоя могут импортировать друг друга.
- Слайсы на одном слое **НЕ** импортируют друг друга (entities↔entities, features↔features — запрещено).
- Кросс-импорт типов между entities возможен через `@x/` нотацию (минимизировать).

### Назначение слоёв

| Слой         | Назначение                                                        | Когда выносить                                                                                              |
| ------------ | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| **shared**   | UI kit, утилиты, api-клиент. БЕЗ бизнес-логики                    | Код не зависит от домена                                                                                    |
| **entities** | Бизнес-сущности (audio, settings, platform)                       | Реальная бизнес-концепция                                                                                   |
| **features** | Пользовательские взаимодействия (recording-flow, hotkey-recorder) | Переиспользуется на 2+ страницах                                                                            |
| **widgets**  | Композиция фич в крупные блоки                                    | Функционал страницы нужен на другой странице; ИЛИ композиция фич, которые не могут импортировать друг друга |
| **pages**    | Страницы, организуют всё вместе                                   | Маршрут приложения                                                                                          |
| **app**      | Wiring: entrypoint, plugins, styles                               | App-wide конфигурация                                                                                       |

**Критерий feature:** выносить в features/ ТОЛЬКО если переиспользуется на нескольких страницах. Иначе — держать в page.

**Критерий widget:** нужен когда функционал страницы вырос и его нужно на другой странице, ИЛИ для композиции фич, которые не могут импортировать друг друга. Если блок используется на одной странице — это НЕ widget.

### Именование сегментов

По **назначению** (ui, api, model, lib, config, infra), НЕ по **сущности** (components, hooks, types, utils).

### Public API

- Каждый слайс обязан иметь `index.ts` — public API
- Внешние потребители импортируют ТОЛЬКО через `index.ts`
- Внутри слайса — relative imports напрямую (НЕ через свой index.ts, это вызывает circular imports)
- Между слайсами — абсолютные импорты через public API (`~/entities/audio`)
- Без wildcard exports (`export * from ...`)

### Layouts в проекте

- Layouts живут в `shared/ui/layouts/` — headless shells со слотами
- Layout НЕ импортирует из верхних слоёв (entities, features, widgets)
- Entity-зависимый контент (voice status, user info) прокидывается через named slots из `app.vue`
- Navigation tabs (useRoute/useRouter) — framework primitives, допустимы в shared/

### Типы

- Доменные типы → `model/types.ts` в соответствующем слайсе
- НЕ экспортировать типы из `.vue` файлов — выносить в model/types.ts
- UI-типы (props) допустимы внутри компонента

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
- **Компоненты из shared:** импортируются явно через barrel `index.ts` (auto-import отключен)
- **DevServer:** http://0.0.0.0:4730 (без HTTPS)
- **Overlay window:** второе Tauri-окно (`label: "overlay"`), transparent, always-on-top, click-through, no decorations. Capabilities в `capabilities/overlay.json`. `tauri-plugin-window-state` denylists "overlay" (prevents corrupted state restore)

---

## Conventions

### Naming

**Компоненты:**

- `u-*` — NuxtUI
- `C*` — Импортируемые компоненты (PascalCase, explicit imports через barrel `index.ts`)

**Store методы:**

- `_privateMethod` или `__internalMethod` — приватные/внутренние
- Публичные computed — без префикса: `isStandby`, `isNativePlatform`
- `readonly()` для защиты состояния от прямого изменения

**Типы:**

- PascalCase: `AudioMode`, `CurrentNativePlatform`, `WakeAction`

### Rust Backend (src-tauri/)

**Tauri events (Rust → Frontend):**

- Именование: `audio-state-changed`, `vad-state`, `transcription`, `wake-command`, `audio-error`, `hotkey-pressed`, `hotkey-released`, `models-status-changed`
- `vad-amplitude` — RMS amplitude per VAD frame (для overlay waveform)
- `stt-engine-resolved` — какой STT engine был фактически выбран
- `subcommand-match` — subcommand recognized (command, action, confidence, tier, params)
- `subcommand-timeout` — AwaitingSubcommand timed out (10s)
- `overlay-capability-degraded` — overlay feature failed (Wayland): `{ capability: string, error: string }`
- Ошибки аудио-пайплайна — событие `audio-error` (НЕ generic `error`)

**PipelineHandle (managed state):**

- Поля приватные, доступ только через методы
- `transition_mode(app, new_mode)` — единственный способ менять mode (атомарно: set + emit). Прямая запись в mode запрещена — гарантирует sync между Rust и frontend
- `join_threads()`, `push_thread()`, `set_trans_tx()`, `request_reload_references()` — safe mutex access (без unwrap, через map_err, все возвращают `Result`)

**Domain types:**

- Все payload/model структуры (`AudioMode`, `ModelInfo`, `ModelStatus`, `WakeCommand`, `RecordResult`, `WakeCommandInfo`, etc.) живут в `audio/mod.rs`
- `commands.rs` — только Tauri command handlers + platform-specific logic (`get_models_dir`). Без domain types, без бизнес-логики
- `capture.rs` — `AudioCapture`, `AudioResampler`, `capture_speech_vad()` (shared VAD capture loop)
- `embedding.rs` — `EmbeddingExtractor`, DTW utilities, `save_frames_file()`/`load_frames_file()`
- `stt.rs` — `DictationEngine` trait, `WhisperEngine`, `ParakeetEngine`, `resolve_stt_engine()`

**Model validation:**

- `stt.rs` валидирует Whisper модель при загрузке: distil-\* модели English-only, использование с `language != "en"` → ошибка
- Валидация по имени файла (convention: tiny/base/small/medium/large-v*/turbo = multilingual, distil-* = English-only)

**Text injection (paste.rs):**

- X11: `Ctrl+V` (стандартный GUI paste)
- Wayland: `Ctrl+Shift+V` + log::warn о возможной проблеме с non-English раскладкой
- macOS: `Cmd+V`, Windows: `Ctrl+V` (с release held modifiers)

**Serde & wire format stability:**

- Enum'ы с serde НЕ используют `rename_all` — каждый вариант имеет явный `#[serde(rename = "...")]`
- Это развязывает имя Rust-варианта и сериализованное значение: переименование варианта не ломает данные на диске / frontend
- При переименовании старого значения — добавлять `#[serde(alias = "old_name")]` для обратной совместимости
- Snapshot-тесты проверяют стабильность wire format: `audio/mod.rs` (AudioMode, WakeAction), `system/detect.rs` (GpuVendor, DisplayServer), `input/mod.rs` (TextInputMethod). Если тест упал, значит изменился формат, нужна миграция

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
- `AudioMode` enum: Off, Standby, Dictation, Processing, AwaitingSubcommand
- Computed: `isOff`, `isStandby`, `isDictation`, `isProcessing`, `isAwaitingSubcommand`

**settings** (`entities/settings/`):

- Включает `sttEngine: SttEngine` (`'auto'` | `'parakeet'` | `'whisper'`)

**wake-commands** (`entities/wake-commands/`):

- Pinia store: `useWakeCommandsStore()`
- CRUD голосовых команд: create, record, delete, rename
- Types: `WakeCommandInfo`, `BaseCommandDef`, `WakeActionOption`
- Lib: `buildBaseCommandName()`, `getBaseCommandNames()` в `lib/builders.ts`

**shared/model/** (`shared/model/`):

- Wire-format types shared across entities: `WakeAction`, `RecordResult`
- Barrel `index.ts` re-exports all types

---

## Commands

```bash
# Development
pnpm dev             # Nuxt dev server (http://0.0.0.0:4730)
pnpm tauri:dev       # Tauri + Nuxt dev

# Build
pnpm build           # Nuxt build
pnpm tauri:build     # Tauri production build

# Lint (use --quiet for minimal output)
pnpm exec eslint --quiet .
pnpm format          # Prettier
```

---

## Important Rules

### Docs Sync Protocol

**При изменении pipeline поведения, state machine, моделей, persistence — обновлять ВСЕ затронутые документы в том же коммите:**

- `CLAUDE.md` — project spec (authoritative)
- `.claude/docs/audio-pipeline.md` — pipeline diagram, models table
- `.claude/docs/evolution-plan.md` — architecture decisions, model choices
- `README.md` — user-facing model download instructions

Stale docs = stale decisions = bugs from misalignment.

### UX/UI Protocol

**Все решения по UX/UI должны быть согласованы с пользователем перед реализацией.** Не придумывать и не додумывать: layout, copy, поведение, анимации, баннеры, flow. Если спецификации нет в этом файле — спросить перед реализацией. В roadmap такие фичи отмечены как "requires UX design".

---

## Important Notes

- SSR отключен — приложение работает как SPA
- Tauri конфиги: `tauri.conf.json5` (base), `tauri.windows.conf.json5`, `tauri.linux.conf.json5`. **Platform конфиги — delta-only** (содержат только overrides, Tauri 2 deep-мержит поверх base)
- Tauri window: 800x600 (min 600x400, resizable)
- Компоненты из shared импортируются явно через barrel `index.ts` (auto-import отключен)
- Layouts в `shared/ui/layouts`
- Модели хранятся в ASCII-путях (Windows: `C:\creo-data\`, Linux: `~/.local/share/creo/`)
- Порт dev server: 4730
- System tray: hide-to-tray при закрытии окна, "Quit" для выхода. Cargo features: `tray-icon`, `image-ico` в tauri dependency

### Build Dependencies (Rust)

- **LLVM/Clang** — требуется для `whisper-rs-sys` (bindgen). Windows: `winget install LLVM.LLVM`, задать `LIBCLANG_PATH="C:/Program Files/LLVM/bin"`
- **CMake** — требуется для `whisper-rs-sys` (компиляция whisper.cpp). Windows: `winget install Kitware.CMake`, добавить в PATH

---

## Roadmap

Единый источник правды — **[README.md → Roadmap](README.md#roadmap)**. Все статусы, детали и "requires design" маркеры ведутся там.

**При реализации фичи — обновить roadmap в README.md** (статус `done`, добавить детали). При добавлении новой фичи — добавить строку в соответствующую секцию.
