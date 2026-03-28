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

Speech-to-text model for dictation (placeholder until ct2rs/parakeet-rs integration).

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
Microphone (cpal) → Resample 48kHz→16kHz (rubato) → Silero VAD (ort/ONNX)
    → speech detected → buffer → Whisper (whisper-rs) → wake word match / dictation text
    → Tauri events → Vue frontend
```

Three threads: audio capture, VAD processing, whisper transcription. Connected via crossbeam channels.

## Roadmap

Statuses: `done`, `in-progress`, `planned`, `requires design` (UX/UI must be agreed before implementation).

<details>
<summary><b>MVP (Audio Pipeline)</b> — done</summary>

| Feature                            | Status | Details                                                                   |
| ---------------------------------- | ------ | ------------------------------------------------------------------------- |
| cpal capture + rubato resampling   | done   | 48kHz→16kHz mono f32                                                      |
| Silero VAD (ort/ONNX)              | done   | 512-sample chunks, threshold 0.5                                          |
| whisper-rs transcription           | done   | Base model (~150MB) as placeholder for both wake word and dictation       |
| Wake word fuzzy matching (strsim)  | done   | 3 commands: приём, вписывай, готово                                       |
| Pipeline orchestration (3 threads) | done   | Processing + Transcription + Capture, crossbeam channels                  |
| Tauri IPC (events + commands)      | done   | start/stop_listening, test_capture, check_models                          |
| Model check + banner               | done   | check_models command, platform-aware paths, UI banner when models missing |
| Frontend state sync                | done   | Pinia store + Tauri event listeners                                       |
| Basic pulse indicator              | done   | Pulse animation when not idle                                             |

</details>

<details>
<summary><b>Post-MVP — Rust Backend</b></summary>

| Feature                      | Status  | Dependencies         | Details                                                                                                                                                |
| ---------------------------- | ------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ct2rs (CTranslate2)          | planned | —                    | Main STT for NVIDIA GPU + CPU. Replaces whisper-rs for dictation. Rust crate: `ct2rs`                                                                  |
| parakeet-rs (Parakeet TDT)   | planned | —                    | Main STT for AMD/Intel GPU + CPU. ONNX model ~600MB. Better Russian WER. Rust crate: `parakeet-rs`                                                     |
| STT engine trait/abstraction | planned | ct2rs or parakeet-rs | Common interface for swapping engines. Current `Transcriber` struct is the abstraction point                                                           |
| enigo text injection         | planned | —                    | Hybrid: SendInput <100 chars, clipboard+paste for longer. **Requires design:** input mode setting (auto / always type / always paste)                  |
| Sound feedback (rodio/cpal)  | planned | —                    | **Requires design:** which sounds, on which events (wake word recognition? start/stop dictation?)                                                      |
| Kando integration            | planned | —                    | **Requires design:** launch mechanism (shell command? hotkey? IPC?)                                                                                    |
| Hotkey fallback              | planned | —                    | **Requires design:** which key, configurability, global hotkey via Tauri                                                                               |
| Model download mechanism     | planned | —                    | **Requires design:** download progress UI, sources, checksum verification, retry, offline fallback (user brings own models)                            |
| Configurable model paths     | planned | —                    | Canonical paths already used (Windows `C:\creo-data\models\`, Linux `~/.local/share/creo/models/`). This feature is about UI settings for custom paths |
| Whisper tiny for wake word   | planned | —                    | Currently base (~150MB), target tiny (~75MB). Switch after pipeline stabilization                                                                      |

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

| Feature                             | Status          | Details                                                                                                       |
| ----------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------- |
| Overlay indicator (separate window) | requires design | Transparent, always-on-top, click-through. Pulse wave visible in peripheral vision, doesn't block interaction |
| Circular waveform (dictation)       | requires design | Compact circular indicator, not a wide rectangle                                                              |
| Subtle idle indicator               | requires design | Barely noticeable, like OpenWhispr                                                                            |
| Settings page                       | requires design | Scope: STT engine, text input mode, history retention, hotkey, model management                               |
| History UI                          | requires design | Command/dictation log with configurable retention (days). Accessible from settings and on first launch        |

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
