# Creo

Desktop voice assistant. Fully offline, all ML models run locally.

Nuxt 3 + Tauri 2 + Rust audio pipeline.

[Prerequisites](#prerequisites) · [Models](#models) · [Development](#development) · [Architecture](#architecture) · [Roadmap](#roadmap)

## Prerequisites

- **Node.js** 18+
- **pnpm** (`npm install -g pnpm`)
- **Rust** (rustup.rs)
- **LLVM** — needed by whisper-rs for C bindings
    - Windows: `winget install LLVM.LLVM`
    - Linux: `sudo apt install llvm-dev libclang-dev` (or equivalent)
- **CMake** — needed by whisper-rs to compile whisper.cpp
    - Windows: `winget install Kitware.CMake`
    - Linux: `sudo apt install cmake`

### Fedora 43 / GCC 15

whisper.cpp crashes with `std::bad_alloc` during build on GCC 15 ([Bug 86164](https://gcc.gnu.org/bugzilla/show_bug.cgi?id=86164)). Fixed in GCC 16 (Fedora 44). Workaround — build with Clang:

```bash
CC=clang CXX=clang++ pnpm tauri:dev
```

Or create `src-tauri/.cargo/config.toml`:

```toml
[env]
CC = "clang"
CXX = "clang++"
```

### Environment variables (Windows)

```bash
# Add to shell profile or set before building
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
export PATH="/c/Program Files/CMake/bin:$PATH"
```

## Setup

```bash
pnpm install
```

## Models

The audio pipeline requires ML models. Download them and place into the models directory:

- **Windows:** `C:\creo-data\models\`
- **Linux:** `~/.local/share/creo/models/`

Create the directory if it doesn't exist.

### Silero VAD v6 (~1.8 MB)

Voice Activity Detection. Detects when someone is speaking.

**Download:** go to https://github.com/snakers4/silero-vad/tree/master/src/silero_vad/data and download `silero_vad.onnx`, then **rename** it to `silero_vad_v6.onnx` and place in the models directory.

Or via command line (Windows):

```bash
mkdir "C:\creo-data\models"
curl -L -o "C:\creo-data\models\silero_vad_v6.onnx" "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx"
```

Linux:

```bash
mkdir -p ~/.local/share/creo/models
curl -L -o ~/.local/share/creo/models/silero_vad_v6.onnx "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx"
```

### Mel Spectrogram (~1 MB) + Speech Embedding (~1.3 MB)

Wake word detection models (Google speech-embedding, from openWakeWord releases).

**Download from:** https://github.com/dscripka/openWakeWord — models `melspectrogram.onnx` and `embedding_model.onnx`.

```bash
curl -L -o "C:\creo-data\models\melspectrogram.onnx" "https://github.com/dscripka/openWakeWord/raw/main/openwakeword/resources/models/melspectrogram.onnx"
curl -L -o "C:\creo-data\models\embedding_model.onnx" "https://github.com/dscripka/openWakeWord/raw/main/openwakeword/resources/models/embedding_model.onnx"
```

### Whisper Base GGML (~150 MB)

Speech-to-text model for dictation (fallback STT, placeholder until parakeet-rs integration).

**Download:** go to https://huggingface.co/ggerganov/whisper.cpp/tree/main and download `ggml-base.bin`.

```bash
curl -L -o "C:\creo-data\models\ggml-base.bin" "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
```

### Verification

After downloading, the models directory should look like:

```
models/
├── silero_vad_v6.onnx    (~1.8 MB)  — voice activity detection
├── melspectrogram.onnx   (~1 MB)    — wake word preprocessing
├── embedding_model.onnx  (~1.3 MB)  — wake word embeddings
└── ggml-base.bin         (~150 MB)  — dictation (whisper base)
```

When the app starts, a banner will show if any models are missing, with the expected path.

## Development

```bash
# Frontend only (web browser)
pnpm dev

# Full app (Tauri + frontend) — requires models
pnpm tauri:dev
```

### Testing the audio pipeline

1. Place both models in the correct directory (see above)
2. Run `pnpm tauri:dev`
3. The app should show "Creo" in idle state with no "Models required" banner
4. Click **Start** to begin listening
5. Say **"Creo, priyom"** (Крео, приём) — should briefly show "Processing" then return to "Listening"
6. Say **"Creo, vpisyvay"** (Крео, вписывай) — enters dictation mode
7. Speak — transcribed text appears in the UI
8. Say **"Creo, gotovo"** (Крео, готово) — returns to listening

### Test capture (no models needed)

In dev mode, there's a "Test Capture (3s)" button that records 3 seconds of microphone audio and shows RMS levels. This verifies that the microphone and resampling work without needing any models.

## Build

```bash
pnpm tauri:build
```

## Architecture

```
Microphone (cpal) → Resample 48kHz→16kHz (rubato) → Silero VAD v6 (ort/ONNX)
    → speech detected → buffer
        → [wake words]: Google speech-embedding 96-dim + DTW frame-level matching
        → [dictation]:  whisper-rs base (placeholder until parakeet-rs)
    → Tauri events → Vue frontend
```

Two pipeline threads: processing (VAD + cpal capture internally) and transcription (DTW + STT). Connected via crossbeam channels.

## Roadmap

Statuses: `done`, `in-progress`, `planned`, `requires design` (UX/UI must be agreed before implementation).

<details>
<summary><b>MVP (Audio Pipeline)</b> — done</summary>

| Feature                             | Status | Details                                                                   |
| ----------------------------------- | ------ | ------------------------------------------------------------------------- |
| cpal capture + rubato resampling    | done   | 48kHz→16kHz mono f32                                                      |
| Silero VAD (ort/ONNX)               | done   | 512-sample chunks, threshold 0.5                                          |
| whisper-rs transcription            | done   | Base model (~150MB) as placeholder for both wake word and dictation       |
| Wake word detection (embedding+DTW) | done   | Google speech-embedding + DTW. 3 commands: приём, вписывай, готово        |
| Pipeline orchestration (2 threads)  | done   | Processing (includes cpal capture) + Transcription, crossbeam channels    |
| Tauri IPC (events + commands)       | done   | start/stop_listening, test_capture, check_models                          |
| Model check + banner                | done   | check_models command, platform-aware paths, UI banner when models missing |
| Frontend state sync                 | done   | Pinia store + Tauri event listeners                                       |
| Basic pulse indicator               | done   | Pulse animation when not idle                                             |

</details>

<details>
<summary><b>Post-MVP — Rust Backend</b></summary>

| Feature                         | Status   | Dependencies | Details                                                                                                                                                |
| ------------------------------- | -------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| STT engine selector             | done     | —            | Backend + persistence (auto/parakeet/whisper). Frontend settings type. UI card in settings deferred (blocked by auto-config UX)                        |
| Subcommand cascade architecture | done     | —            | embedding.rs, SubcommandTier trait, DtwTier (Tier 1), SubcommandCascade, capture_speech_vad() shared VAD loop                                          |
| parakeet-rs (Parakeet TDT)      | planned  | —            | Primary STT. ONNX Runtime (CUDA/DirectML/CPU). ~640MB INT8. Best Russian WER, native punctuation. Rust crate: `parakeet-rs`                            |
| ct2rs (CTranslate2)             | deferred | —            | Отложен до реализации всех основных фич. Для оптимизации пограничных конфигураций (Intel CPU-only). Блокеры в `.claude/docs/audio-pipeline.md`         |
| Text injection (Paste/Type)     | done     | —            | Paste (arboard clipboard + Ctrl+V) and Type (enigo char-by-char), user-selectable in settings. **Planned:** hybrid auto-switching by text length       |
| Sound feedback (rodio/cpal)     | planned  | —            | **Requires design:** which sounds, on which events (wake word recognition? start/stop dictation?)                                                      |
| Kando integration               | planned  | —            | **Requires design:** launch mechanism (shell command? hotkey? IPC?)                                                                                    |
| Hotkey fallback                 | planned  | —            | **Requires design:** which key, configurability, global hotkey via Tauri                                                                               |
| Model download mechanism        | planned  | —            | **Requires design:** download progress UI, sources, checksum verification, retry, offline fallback (user brings own models)                            |
| Configurable model paths        | planned  | —            | Canonical paths already used (Windows `C:\creo-data\models\`, Linux `~/.local/share/creo/models/`). This feature is about UI settings for custom paths |

</details>

<details>
<summary><b>Post-MVP — Auto-Configuration</b></summary>

| Feature                     | Status          | Details                                                                                             |
| --------------------------- | --------------- | --------------------------------------------------------------------------------------------------- |
| Hardware detection          | planned         | GPU vendor/VRAM, CPU, RAM                                                                           |
| Engine/model recommendation | planned         | Optimal engine + model + quantization based on hardware                                             |
| First-launch wizard         | requires design | How to present recommendation, how user overrides. Non-technical, user-friendly — no GPU/CPU jargon |

</details>

<details>
<summary><b>Post-MVP — UX / Frontend</b></summary>

| Feature                             | Status          | Details                                                                                                                                                           |
| ----------------------------------- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Overlay indicator (separate window) | done            | Transparent always-on-top click-through window. State-driven circle (standby/dictation/processing/awaiting), waveform bars, processing ring, transient animations |
| System tray                         | done            | Tray icon, Show Dashboard / Quit menu, hide-to-tray on close                                                                                                      |
| Dev controls in Settings            | done            | Devtools suppression + click-through toggle for overlay window                                                                                                    |
| Circular waveform (dictation)       | requires design | Compact circular indicator, not a wide rectangle                                                                                                                  |
| Subtle idle indicator               | requires design | Barely noticeable, like OpenWhispr                                                                                                                                |
| Settings page                       | requires design | Scope: STT engine, text input mode, history retention, hotkey, model management                                                                                   |
| History UI                          | requires design | Command/dictation log with configurable retention (days). Accessible from settings and on first launch                                                            |

</details>

<details>
<summary><b>Post-MVP — Banners / Guides</b></summary>

| Banner                | Status          | Platform | Details                                                                        |
| --------------------- | --------------- | -------- | ------------------------------------------------------------------------------ |
| Admin elevation       | requires design | Windows  | Consequences of running without admin (UIPI: can't type into elevated windows) |
| Model guide           | requires design | All      | First-launch model/engine selection guide                                      |
| Cyrillic path warning | requires design | Windows  | Warning + autofix for non-ASCII user paths                                     |
| Wayland limitations   | requires design | Linux    | Notice about clipboard+paste fallback for text injection                       |

</details>

<details>
<summary><b>Future — macOS</b></summary>

| Feature                    | Details                                                         |
| -------------------------- | --------------------------------------------------------------- |
| macOS support              | Accessibility + Microphone permissions, Notarization, Metal GPU |
| Apple Silicon optimization | Metal backend for ONNX Runtime and whisper.cpp                  |

</details>

<details>
<summary><b>Known Issues / In Progress</b></summary>

| Issue                        | Details                                                                                 |
| ---------------------------- | --------------------------------------------------------------------------------------- |
| Overlay positioning offset   | Windows invisible borders (WS_THICKFRAME) cause ~24px offset. Win32 API fix planned     |
| Proximity fade               | Not yet implemented. Requires Rust cursor polling to fade overlay when mouse approaches |
| Batch dictation accumulation | Not yet implemented. Dictation segments should accumulate before injection              |

</details>
