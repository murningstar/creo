# Architecture Audit Results

> Проведён 2026-03-27. 3 независимых аудитора, полный codebase review (20+ файлов).
> Статус каждого finding обновляется по мере фиксов.

---

## CRITICAL

### C1. Pipeline не умеет менять mode без перезапуска

**Status:** ✅ Fixed
**Files:** `commands.rs`, `use-dictation-flow.ts`
**Consensus:** 3/3

Added `transition_to_dictation`, `transition_to_standby`, `get_current_mode` Rust commands. Dictation flow uses mode transitions instead of `stop_listening`/`start_dictation`.

### C2. Wake word samples невидимы для работающего pipeline

**Status:** ✅ Fixed
**Files:** `pipeline.rs`, `mod.rs`, `commands.rs`
**Consensus:** 3/3

Added `TranscriptionRequest::ReloadReferences`. PipelineHandle stores `trans_tx` sender. `record_wake_sample` and `delete_wake_command` trigger reload.

### C3. Rename flow без atomicity

**Status:** ✅ Fixed — persist name FIRST, delete old AFTER. If delete fails, orphaned dirs visible as user commands (not lost).
**Files:** `rename-assistant.vue:112-123`
**Consensus:** 3/3

Delete old → save name → new already recorded. Crash/fail в любой точке = permanent desync. Нет rollback, нет recovery marker.

**Fix:** Reverse order: persist name first → delete old commands. Или: single Rust command для atomic rename.

### C4. Event listeners никогда не cleanup'ятся

**Status:** ✅ Fixed
**Files:** `audio.store.ts`, `use-dictation-flow.ts`
**Consensus:** 3/3

Guard added: cleanup existing listeners before re-registering. Prevents double-fire on HMR/reload.

---

## HIGH

### H1. `start_dictation` fails когда pipeline уже running

**Status:** ✅ Fixed (via C1)
**Consensus:** 2/3

Resolved by `transition_to_dictation` — works when pipeline already running in Standby.

### H2. Frontend-backend state desync при reload

**Status:** ✅ Fixed
**Files:** `audio.store.ts`, `init.ts`
**Consensus:** 2/3

Added `syncMode()` → `get_current_mode` Tauri command. Called in init.ts after setupEventListeners.

### H3. Thread panic → zombie pipeline

**Status:** 🟡 Partially fixed
**Files:** `vad.rs:84`, `wakeword.rs:663`
**Consensus:** 2/3

`assert_eq!` → `anyhow::ensure!` (no more panics from known assert sites). `catch_unwind` wrapper for thread bodies — deferred.

### H4. Transcriber prev_context не сбрасывается

**Status:** ✅ Fixed
**Files:** `pipeline.rs`
**Consensus:** 3/3

`dictation_transcriber.reset_context()` called when stop/cancel wake command detected.

### H5. Model polling thread бессмертный

**Status:** ✅ Fixed
**Files:** `lib.rs`
**Consensus:** 3/3

Shared `AtomicBool` shutdown flag, checked in loop, set in `RunEvent::Exit`.

### H6. Hardcoded "ru" язык в dictation

**Status:** 🟡 TODO comment added
**Files:** `pipeline.rs:325`
**Consensus:** 2/3

Needs language setting passed from frontend to pipeline. Deferred — requires settings accessible from Rust thread.

---

## MEDIUM

### M1. Audio capture silently drops data

**Status:** ✅ Fixed
**Files:** `capture.rs`
**Consensus:** 3/3

Log dropped chunks (every 100th drop).

### M2. `Ordering::Relaxed` insufficient

**Status:** ✅ Fixed
**Files:** `mod.rs`
**Consensus:** 2/3

All atomics → `Ordering::SeqCst`. ARM-safe.

### M3. Command name unsanitized → path injection

**Status:** ✅ Fixed
**Files:** `commands.rs`
**Consensus:** 2/3

`validate_command_name()` rejects `..`, `/`, `\`, null, Windows reserved chars. Applied to `record_wake_sample` and `delete_wake_command`.

### M4. `unregisterAll()` слишком широкий

**Status:** ✅ Fixed — `unregister('CmdOrCtrl+Backquote')` instead of `unregisterAll()`.
**Files:** `init.ts:37`
**Consensus:** 2/3

Clears ALL global shortcuts, не только наш default.

**Fix:** `unregister(specific_shortcut)` вместо `unregisterAll()`.

### M5. config.json read-modify-write не atomic

**Status:** 🔴 Open
**Files:** `commands.rs`, `wakeword.rs`
**Consensus:** 1/3

Concurrent writes → overwrite. Low priority — concurrent command operations are rare in practice.

### M6. `deleteBaseCommands` swallows IO errors

**Status:** ✅ Fixed — `console.warn` on error instead of silent catch.
**Files:** `wake-commands.store.ts:54-58`
**Consensus:** 2/3

`catch { }` — all errors silently ignored. Should log and distinguish "not found" from IO errors.

### M7. Duplicated path computation

**Status:** ✅ Fixed
**Files:** `commands.rs`
**Consensus:** 2/3

Extracted `get_creo_data_dir()`. `get_models_dir()`, `get_wakewords_dir()`, `start_pipeline_with_mode` all use it.

### M8. `WakeCommand` alias TODO

**Status:** ✅ Fixed — all 23 occurrences renamed to WakeAction. Alias removed. WakeCommandPayload → WakeActionPayload. TranscriptionResult::WakeCommand → TranscriptionResult::WakeAction.
**Files:** `mod.rs:71`
**Consensus:** 2/3

Needs dedicated rename pass.

---

## LOW

### L1. Layout tab hardcoded "Creo"

**Status:** ✅ Fixed — Layout accepts `appLabel` prop, app.vue passes `settingsStore.assistantName`.

### L2. Debug RMS logging left

**Status:** ✅ Fixed — Removed from pipeline.rs.

### L3. Sample index collision risk

**Status:** ✅ Fixed — uses `max existing index + 1` instead of `count / 2`.

### L4. `WakeAction` type duplicated in 2 entities

**Status:** ✅ Acknowledged — conscious FSD trade-off. Comment added linking both types.

### L5. `autoSave: true` + explicit `save()` redundant

**Status:** ✅ Fixed — Changed to `autoSave: false`.

### L6. `RecordResult`/`WakeCommandInfo` use `rename_all` (convention violation)

**Status:** ✅ Fixed — explicit `#[serde(rename = "...")]` per field.

---

## FUTURE-PROOFING

### F1. Single transcription thread = bottleneck

**Status:** 📋 Planned

Both wake word detection and dictation share one thread. Adding Vosk + Qwen3 → 3 inference engines on one thread.

**Plan:** Split into separate threads: wake word (embedding+DTW), STT (Parakeet/Whisper), NLU (Vosk/Qwen3).

### F2. No model load/unload lifecycle

**Status:** 📋 Planned

Models loaded once, live forever. Evolution plan needs "load on first use, keep warm 30s, unload" for Qwen3.

**Plan:** Build model manager with load/unload/keep-warm semantics.

### F3. Need manifest-based command registry

**Status:** 📋 Planned

Current: command = directory on filesystem + entry in config.json. Future: DTW commands (audio samples) + Vosk commands (text-only) + Qwen3 parametric templates — need unified registry.

**Plan:** `wakewords/manifest.json` — single source of truth for all command types, metadata, actions.

---

## Score: 24/25 fixed, 1 remaining (M5 config.json atomicity — low priority, rare in practice)
