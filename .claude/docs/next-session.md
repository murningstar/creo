# Next Session Handoff — 2026-03-29

## Что было сделано в этой сессии

### 1. STT Engine Selector (backend + persistence)

- Rust: `resolve_stt_engine(preference)` заменил `detect_stt_engine()`, принимает `"auto"/"parakeet"/"whisper"`
- `start_listening`/`start_dictation` принимают `stt_engine: Option<String>`
- Emits `stt-engine-resolved` event с фактически выбранным движком
- Frontend: `SttEngine` тип + persistence в settings store
- **UI карточка в Settings НЕ реализована** — blocked by auto-config UX проработка

### 2. Subcommand Cascade Architecture

- `embedding.rs` — shared `EmbeddingExtractor` извлечён из `wakeword.rs`
- `subcommand.rs` — `SubcommandManifest`, `SubcommandDef` (dtw/vosk/llm tiers), `ParametricTemplate`, `SlotDef`, `SubcommandTier` trait, `DtwTier`, `SubcommandCascade`
- Pipeline: `SubcommandCheck` request, cascade в transcription thread, wake words проверяются первыми в AwaitingSubcommand, 10s timeout → Standby
- 4 Tauri commands: `get_subcommands`, `create_subcommand`, `delete_subcommand`, `record_subcommand_sample`
- Frontend: `entities/subcommands/` (types + Pinia store + public API)
- Audio store: `SubcommandMatchEvent` + `subcommand-timeout` listeners

### 3. Overlay Indicator

- Tauri: второе окно (transparent, always-on-top, click-through, no decorations, `shadow: false`)
- Capabilities: `overlay.json`
- Nuxt: overlay layout + `/overlay` page
- Визуальные состояния: Standby (breathing glow), Dictation (waveform bars), AwaitingSubcommand (⌘ icon), Processing (conic ring), Success/Error (SVG animations), Mini-badge (batch processing)
- `vad-amplitude` event из Rust для waveform bars
- `tauri-plugin-window-state` denylists overlay (prevents position corruption)

### 4. System Tray

- Tray icon с меню: "Show Dashboard" / "Quit"
- Закрытие Dashboard → hide to tray (pipeline продолжает работать)
- Cargo features: `tray-icon`, `image-ico`

### 5. Dev Controls

- Settings card (видна только в `import.meta.dev`)
- "Suppress devtools on overlay" toggle → `emitTo('overlay', 'overlay-suppress-devtools', bool)`
- "Click-through" toggle → `emitTo('overlay', 'overlay-set-click-through', bool)`
- MutationObserver в overlay page убирает Vite error overlay + vite-plugin-checker overlay + Nuxt devtools
- `init.ts` plugin пропускается в overlay window (`isMainWindow()` guard)

---

## Что нужно сделать дальше (приоритет)

### Немедленно (overlay polish):

1. **Overlay positioning fix** — Windows invisible borders (WS_THICKFRAME) вызывают ~24px offset. Решение: Win32 API через `windows-sys` crate — `SetWindowLongPtrW` убрать `WS_THICKFRAME`. ~10 строк platform-specific кода.
2. **Cursor proximity fade** — Rust polling thread (20Hz) → `cursor_position()` → compute distance → `emit_to("overlay", "cursor-proximity", f64)`. Overlay opacity = 1.0 - proximity \* 0.8.
3. **Error click-through toggle** — при `audio-error` Rust отключает click-through на overlay, после dismiss включает обратно.
4. **Corner position setting** — `OverlayCorner` type в settings, Rust repositions overlay based on preference.
5. **Batch dictation accumulation** — accumulate batches в `Vec<String>`, вставка в конце по умолчанию (опция инкрементальной вставки в settings).

### Следующие задачи (из roadmap):

6. **Vosk integration** (Tier 2) — `vosk-rs`, grammar mode, `[unk]` rejection
7. **Qwen3 1.7B integration** (Tier 3) — `llama-cpp-2` + GBNF, dynamic system prompt, parametric command UI
8. **Auto-config + Wizard** — system detection → model recommendations → download with progress → wake word recording
9. **History persistence** — backend storage + UI list
10. **Sound feedback** — rodio sounds
11. **Hybrid text injection** — auto paste/type

---

## Известные проблемы

- **Overlay position offset (~24px)** — invisible borders on Windows. Fix: Win32 API `SetWindowLongPtrW` to remove `WS_THICKFRAME`
- **Wayland** — click-through и always-on-top ненадёжны. Fallback: XWayland
- **vite-plugin-checker overlay** — удаляется MutationObserver, но может появиться при первой загрузке до mount
- **DevControls toggles** — работают через `emitTo`, нужен перезапуск dev server после изменений в Rust

---

## Ключевые архитектурные решения (context)

- **3-tier cascade** — validated industry consensus (2026 research). DTW → Vosk → Qwen3+GBNF.
- **Overlay = primary feedback** — пользователь взаимодействует голосом, overlay подтверждает. Dashboard вторичен.
- **Вставка в конце по умолчанию** — батчи копятся, вставляются по "готово". Опция инкрементальной вставки. Причина: возможность LLM post-processing.
- **Off mode не нужен в production** — Creo всегда слушает (минимум Standby). Off = закрытое приложение.
- **Quality > model size** — при auto-config рекомендовать качественную модель, fallback на лёгкую только если hardware не тянет.

---

## Файлы исследований (в `.claude/plans/`)

- `cosmic-singing-zephyr-agent-a82932058b4a12ef8.md` — waveform visualization research
- `cosmic-singing-zephyr-agent-aef064f2b43e0bbc9.md` — Tauri overlay window research
- `cosmic-singing-zephyr-agent-ab8b89f68c7689d5d.md` — starburst/CSS animation research
- `cosmic-singing-zephyr-agent-a9121d9cc113ba687.md` — voice command architecture 2026 research (28KB, 50+ sources)
