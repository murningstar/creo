# Next Session Handoff — 2026-04-07

## Что было сделано в этой сессии

### Architecture Audit Refactoring

Полный аудит кодобазы (6 аудиторов + 3 валидатора) → рефакторинг по всем находкам → документация.

**Frontend (FSD):**

- `dictation-flow` перенесён из `features/` → `app/` (app-wiring, 0 page consumers)
- `rename-assistant` перенесён из `widgets/` → `pages/settings/ui/` (single-page usage)
- `WakeAction` и `RecordResult` вынесены в `shared/model/types.ts` (wire-format types)
- `WakeActionType` переименован в `WakeAction` везде
- Barrel `index.ts` для shared/ сегментов (icons, keystroke-recorder, model)
- `hotkey-constraints.ts`: types в `model/`, logic в `lib/`
- `buildBaseCommandName()` / `getBaseCommandNames()` вынесены в `wake-commands/lib/builders.ts`
- `ref()` → plain `let` для external handles (`_unlisten`, `_finishingTimeout`)
- Удалён dead code: `entities/subcommands/`, `action-list`, ghost overlay listeners, `console.log`, `__setCurrentNativePlatform`, `c-*` auto-import config
- Удалён `type-fest`, `--debug` из lint scripts

**Rust backend:**

- `RecordResult`/`WakeCommandInfo` перенесены из `commands.rs` → `audio/mod.rs`
- `resolve_stt_engine` перенесён из `commands.rs` → `audio/stt.rs` (parameterized)
- `save_frames_file`/`load_frames_file` консолидированы в `embedding.rs`
- `capture_speech_vad()` извлечён в `capture.rs` (shared VAD loop)
- Explicit serde renames (3 enum'а) + snapshot тесты
- Удалён `audio/transcriber.rs` (superseded by stt.rs)
- Production logging enabled (Warn level)
- Vulkan instance leak fix (Drop guard)
- Mutex poison handling стандартизирован
- Overlay capabilities trimmed
- Удалены `strsim`, `linfa`, `linfa-svm`

**Документация:**

- CLAUDE.md: полный Rust module inventory, wake-commands entity, Docs Sync Protocol с trigger conditions и authority hierarchy
- README.md: architecture diagram, roadmap, text injection status
- evolution-plan.md: dictation fixes marked DONE, summary chart updated
- 10 противоречий между документами исправлено
- Memory files обновлены

---

## Что нужно сделать дальше (приоритет)

### Немедленно (overlay polish):

1. **Overlay positioning fix** — Windows invisible borders (WS_THICKFRAME) вызывают ~24px offset. Решение: Win32 API через `windows-sys` crate — `SetWindowLongPtrW` убрать `WS_THICKFRAME`
2. **Cursor proximity fade** — Rust polling thread (20Hz) → `cursor_position()` → compute distance → `emit_to("overlay", "cursor-proximity", f64)`
3. **Error click-through toggle** — при `audio-error` Rust отключает click-through на overlay
4. **Corner position setting** — `OverlayCorner` type в settings
5. **Batch dictation accumulation** — accumulate batches в `Vec<String>`, вставка в конце по умолчанию

### Следующие задачи (из roadmap):

6. **Vosk integration** (Tier 2) — `vosk-rs`, grammar mode, `[unk]` rejection
7. **Qwen3 1.7B integration** (Tier 3) — `llama-cpp-2` + GBNF, dynamic system prompt
8. **Auto-config + Wizard** — system detection → model recommendations → download
9. **History persistence** — backend storage + UI list
10. **Sound feedback** — rodio sounds
11. **Hybrid text injection** — auto paste/type by length

---

## Известные проблемы

- **Overlay position offset (~24px)** — invisible borders on Windows
- **Wayland** — click-through и always-on-top ненадёжны. Fallback: XWayland
- **vite-plugin-checker overlay** — удаляется MutationObserver, но может появиться при первой загрузке
- **Hardcoded "ru" language** — `commands.rs:153`, TODO: language should come from settings
- **Compiler warnings** — 4 pre-existing: unused import, dead fields/methods in wakeword.rs

---

## Ключевые архитектурные решения (context)

- **3-tier cascade** — validated industry consensus (2026 research). DTW → Vosk → Qwen3+GBNF.
- **Overlay = primary feedback** — пользователь взаимодействует голосом, overlay подтверждает. Dashboard вторичен.
- **Вставка в конце по умолчанию** — батчи копятся, вставляются по "готово". Опция инкрементальной вставки. Причина: возможность LLM post-processing.
- **Off mode не нужен в production** — Creo всегда слушает (минимум Standby). Off = закрытое приложение.
- **Quality > model size** — при auto-config рекомендовать качественную модель, fallback на лёгкую только если hardware не тянет.
- **CLAUDE.md = authoritative spec** — при противоречии между документами обновляется другой документ, не CLAUDE.md.

---

## Файлы исследований (в `.claude/plans/`)

- `cosmic-singing-zephyr-agent-a82932058b4a12ef8.md` — waveform visualization research
- `cosmic-singing-zephyr-agent-aef064f2b43e0bbc9.md` — Tauri overlay window research
- `cosmic-singing-zephyr-agent-ab8b89f68c7689d5d.md` — starburst/CSS animation research
- `cosmic-singing-zephyr-agent-a9121d9cc113ba687.md` — voice command architecture 2026 research (28KB, 50+ sources)
