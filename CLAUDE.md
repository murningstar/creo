# Creo — Claude Code Context

## Project Overview

Десктопный голосовой помощник (Windows + Linux) на Nuxt 3 + Tauri с Feature-Sliced Design архитектурой.

**Статус:** Ранняя стадия — scaffold проекта, placeholder-структура для аудио-пайплайна.

---

## Concept

**Creo** — голосовой ассистент для десктопа. Основная задача — распознавание голоса, обработка команд, диктовка текста.

### Планируемые возможности

1. **Wake word detection** — активация по ключевому слову
2. **Voice commands** — голосовые команды для управления
3. **Dictation** — диктовка текста
4. **Audio pipeline** — захват микрофона, VAD, STT (Whisper)

---

## Tech Stack

- Nuxt 3 (Vue 3 Composition API, `<script setup>`)
- TypeScript (strict mode)
- Tauri 2 (native desktop: Windows, Linux)
- Pinia (state management)
- NuxtUI (UI components, prefix `u-`)
- TailwindCSS
- VueUse (утилиты)

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
- **DevServer:** http://0.0.0.0:1337 (без HTTPS)

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
pnpm dev             # Nuxt dev server (http://0.0.0.0:1337)
pnpm tauri:dev       # Tauri + Nuxt dev

# Build
pnpm build           # Nuxt build
pnpm tauri:build     # Tauri production build

# Lint (use --quiet for verification, NOT `pnpm lint` — it has --debug)
pnpm exec eslint --quiet .
pnpm format          # Prettier
```

---

## Important Notes

- SSR отключен — приложение работает как SPA
- Tauri конфиги: `tauri.conf.json5`, `tauri.windows.conf.json5`, `tauri.linux.conf.json5`
- Tauri window: 400x600 (компактный формат для voice assistant)
- Auto-import компонентов из shared с префиксом `c-`
- Layouts в `shared/ui/layouts`
