# Next Session Handoff — 2026-04-08

## Что было сделано в этой сессии

### Vosk Tier 2 Integration

Полная интеграция Vosk grammar-constrained STT как Tier 2 в subcommand cascade.

**Rust backend:**

- `VoskTier` struct в `subcommand.rs` — реализует `SubcommandTier` trait, `#[cfg(feature = "vosk")]`
- Grammar из manifest `phrases` + `[unk]` для rejection. Phrase matching case-insensitive
- Audio f32→i16 conversion, word-level confidence aggregation
- Wired в `SubcommandCascade` как optional Tier 2 (DTW → Vosk → future Qwen3)
- Model path threaded через `commands.rs → pipeline.rs → subcommand.rs`
- Graceful skip when model missing or feature disabled
- `check_models` включает Vosk с `optional: true`

**Build setup:**

- `vosk = "0.3"` behind cargo feature `vosk` в Cargo.toml
- `libvosk.so` (v0.3.45, 25MB) в `src-tauri/lib/vosk/` (gitignored, prebuilt from alphacep/vosk-api)
- `build.rs` — link search path + rpath для runtime

**Audit fixes (3 независимых аудитора):**

- `ModelInfo.optional` field — `all_present` считает только required models (не блокирует UI для опциональных)
- `accept_waveform` error logging + early return (было `let _ =`)
- Snapshot test `subcommand_tier_kind_serialization_stability` (convention compliance)
- Stale "— future" comment removed
- No-op `as f32` cast removed
- Duplicate phrase warning on `phrase_map` collision

**Документация:** CLAUDE.md, audio-pipeline.md, evolution-plan.md, README.md обновлены.

---

## Что нужно сделать дальше (приоритет)

### Overlay polish (отложено, не критично для core):

1. **Overlay positioning fix** — Windows invisible borders (WS_THICKFRAME) ~24px offset. Win32 API через `windows-sys`
2. **Cursor proximity fade** — Rust polling thread (20Hz) → `cursor_position()` → emit
3. **Error click-through toggle** — при `audio-error` отключить click-through на overlay
4. **Corner position setting** — `OverlayCorner` type в settings
5. **Batch dictation accumulation** — accumulate batches в `Vec<String>`, вставка в конце

### Следующие задачи (из roadmap):

6. **Qwen3 1.7B integration** (Tier 3) — `llama-cpp-2` + GBNF, dynamic system prompt из user templates
7. **Auto-config + Wizard** — system detection → model recommendations → download
8. **History persistence** — backend storage + UI list
9. **Sound feedback** — rodio sounds
10. **Hybrid text injection** — auto paste/type by length

---

## Расхождения между сессиями (требуют анализа)

### C1: DTW distance threshold — docs vs code

- **`evolution-plan.md:118`** говорит `0.20`, калибровочные данные: "true matches 0.05-0.15, false positives 0.24+"
- **`embedding.rs:29`** в коде `0.15`, калибровочные данные: "true matches ≈ 0.03-0.07, false positives ≈ 0.19+"
- **Причина:** threshold был перекалиброван в коде по реальным записям (более узкие диапазоны), но evolution-plan.md не был обновлён. Документ всё ещё содержит оригинальные research estimates.
- **Действие:** Решить что authoritative — код (0.15) или docs (0.20). Обновить evolution-plan.md если код правильный (вероятнее, т.к. калиброван по реальным данным).

### C2: Frontend ModelInfo missing `optional` field

- **Rust `audio/mod.rs:147-151`**: `ModelInfo` struct имеет `pub optional: bool` с `#[serde(default)]`
- **Frontend `entities/audio/model/types.ts:46-52`**: `ModelInfo` interface **НЕ** имеет поля `optional`
- **Причина:** поле `optional` добавлено в Rust в этой сессии (audit fix F1), frontend type не обновлён
- **Последствия:** Runtime не ломается (JS duck-typing, `serde(default)` = `false`), но TS type неполный — `model.optional` без cast недоступен. `allPresent` корректно считается на backend, frontend просто не видит поле.
- **Действие:** Добавить `optional: boolean` в frontend `ModelInfo` interface. Проверить, используется ли `allPresent` / `optional` где-либо во frontend логике.

### C3: Потерянный коммит `e25cb3a` — platform research notes

- Коммит содержал research findings в CLAUDE.md, был reset away при синхронизации с origin
- Code fixes для тех же проблем реализованы независимо в `a16b3f2`, но документация потеряна:
    - Wayland `virtual-keyboard-v1` protocol limitations
    - CTranslate2 CPU RTF benchmarks (RTF ~1.10 для turbo на i5-12450H)
    - `distil-large-v3` подтверждение непригодности для русского
    - OpenVINO encoder status на Intel CPU
    - Hotkey UX: "prefer single non-symbol key over combos"
- **Действие:** Восстановить findings в соответствующих docs (platform.md, evolution-plan.md, audio-pipeline.md). Часть уже есть в evolution-plan.md (ct2rs benchmarks, distil-large-v3), проверить полноту.

---

## Известные проблемы

- **Overlay position offset (~24px)** — invisible borders on Windows
- **Wayland** — click-through и always-on-top ненадёжны. Overlay skipped (commit 3f2cea8)
- **vite-plugin-checker overlay** — удаляется MutationObserver, но может появиться при первой загрузке
- **Hardcoded "ru" language** — `commands.rs:169`, TODO: language should come from settings
- **Compiler warnings** — 4 pre-existing: unused `GlobalShortcutExt` (lib.rs), dead `current_threshold` (vad.rs), dead fields в `DetectionResult` (wakeword.rs), dead `extract_mean_embedding` (wakeword.rs)
- **`adfsadfsadf`** — мусорный untracked файл в корне репозитория, удалить
- **`src/widgets/`** — пустая директория (только .gitkeep)
- **libvosk production bundling** — rpath в build.rs указывает на dev path. Для release: Tauri `bundle.resources` + `$ORIGIN` rpath или static link
- **Windows vosk.dll** — нет Windows-specific DLL handling в build.rs (нужно при включении vosk на Windows)
- **CLAUDE.md пробелы** — rubato не упомянут в Tech Stack; `app/dictation-flow/` упомянут в tree, но назначение не описано

---

## Заметки для следующей сессии

- **Docs Sync Protocol** (CLAUDE.md) — при ЛЮБОМ изменении кода сверяться с таблицей триггеров
- **overlay.vue — прямой consumer** Tauri events (не через audio store). При изменении event payload — проверять overlay.vue отдельно
- **`cargo test --features vosk`** — обязательно после любых изменений типов с `#[derive(Serialize)]` (snapshot тесты, теперь 8 тестов)
- **Не удалять документы** без личного прочтения
- **Vosk model для тестирования:** `vosk-model-small-ru-0.22` → rename `vosk-model-small-ru` → models dir. Download: alphacephei.com/vosk/models
- **libvosk для dev setup:** `vosk-linux-x86_64-0.3.45.zip` from github.com/alphacep/vosk-api/releases/tag/v0.3.45 → extract `libvosk.so` → `src-tauri/lib/vosk/`

---

## Ключевые архитектурные решения (context)

- **3-tier cascade** — validated industry consensus. DTW → Vosk → Qwen3+GBNF. Tiers 1+2 implemented.
- **Vosk grammar mode** — 0 recordings от пользователя (vs DTW 3-15 samples), scales to 50+ commands, `[unk]` rejection
- **Feature-gated vosk** — `--features vosk` в Cargo, build stays green without libvosk
- **ModelInfo.optional** — optional models не блокируют `all_present` check (UI/auto-start не ломается)
- **Recognizer per call** — не кешируется: grammar может измениться после reload(), overhead negligible для коротких utterances
- **Overlay = primary feedback** — dashboard вторичен
- **Вставка в конце по умолчанию** — батчи копятся, вставляются по "готово"
- **Quality > model size** — рекомендовать качественную модель, fallback на лёгкую если hardware не тянет
- **CLAUDE.md = authoritative spec**

---

## Файлы исследований (в `.claude/plans/`)

- `cosmic-singing-zephyr-agent-a82932058b4a12ef8.md` — waveform visualization research
- `cosmic-singing-zephyr-agent-aef064f2b43e0bbc9.md` — Tauri overlay window research
- `cosmic-singing-zephyr-agent-ab8b89f68c7689d5d.md` — starburst/CSS animation research
- `cosmic-singing-zephyr-agent-a9121d9cc113ba687.md` — voice command architecture 2026 research (28KB, 50+ sources)
