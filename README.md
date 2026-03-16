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

The audio pipeline requires two ML models. Download them and place into the models directory:

- **Windows:** `C:\creo-data\models\`
- **Linux:** `~/.local/share/creo/models/`

Create the directory if it doesn't exist.

### Silero VAD v5 (~1.8 MB)

Voice Activity Detection. Detects when someone is speaking.

**Download:** go to https://github.com/snakers4/silero-vad/tree/master/src/silero_vad/data and download `silero_vad.onnx`, then **rename** it to `silero_vad_v5.onnx` and place in the models directory.

Or via command line (Windows):

```bash
mkdir "C:\creo-data\models"
curl -L -o "C:\creo-data\models\silero_vad_v5.onnx" "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx"
```

Linux:

```bash
mkdir -p ~/.local/share/creo/models
curl -L -o ~/.local/share/creo/models/silero_vad_v5.onnx "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx"
```

### Whisper Base GGML (~150 MB)

Speech-to-text model. Used for wake word detection and dictation (MVP).

**Download:** go to https://huggingface.co/ggerganov/whisper.cpp/tree/main and download `ggml-base.bin`.

Or via command line (Windows):

```bash
curl -L -o "C:\creo-data\models\ggml-base.bin" "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
```

Linux:

```bash
curl -L -o ~/.local/share/creo/models/ggml-base.bin "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
```

### Verification

After downloading, the models directory should look like:

```
models/
├── silero_vad_v5.onnx   (~1.8 MB)
└── ggml-base.bin         (~150 MB)
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

<details>
<summary><b>MVP (Audio Pipeline)</b> — done</summary>

| Feature                            | Status | Details                                             |
| ---------------------------------- | ------ | --------------------------------------------------- |
| cpal capture + rubato resampling   | done   | 48kHz→16kHz mono f32                                |
| Silero VAD (ort/ONNX)              | done   | 512-sample chunks, threshold 0.5                    |
| whisper-rs transcription           | done   | Base model as placeholder                           |
| Wake word fuzzy matching (strsim)  | done   | 3 commands: приём, вписывай, готово                 |
| Pipeline orchestration (3 threads) | done   | Processing + Transcription + Capture                |
| Tauri IPC (events + commands)      | done   | start/stop_listening, get_audio_state, test_capture |
| Model check + banner               | done   | check_models command, platform-aware paths          |
| Frontend state sync                | done   | Pinia store + Tauri event listeners                 |

</details>

<details>
<summary><b>Post-MVP — Rust Backend</b></summary>

| Feature                      | Status  | Details                                                  |
| ---------------------------- | ------- | -------------------------------------------------------- |
| ct2rs (CTranslate2)          | planned | Main STT for NVIDIA GPU + CPU                            |
| parakeet-rs (Parakeet TDT)   | planned | Main STT for AMD/Intel GPU + CPU, ONNX ~600MB            |
| STT engine trait/abstraction | planned | Common interface for swapping engines                    |
| enigo text injection         | planned | Hybrid: SendInput <100 chars, clipboard+paste for longer |
| Sound feedback (rodio/cpal)  | planned | Audio cues for state transitions                         |
| Kando integration            | planned | Launch mechanism for command mode                        |
| Hotkey fallback              | planned | Global hotkey as wake word alternative                   |
| Model download mechanism     | planned | Auto-download with progress UI                           |
| Whisper tiny for wake word   | planned | Switch from base (~150MB) to tiny (~75MB)                |

</details>

<details>
<summary><b>Post-MVP — Auto-Configuration</b></summary>

| Feature                     | Status          | Details                                  |
| --------------------------- | --------------- | ---------------------------------------- |
| Hardware detection          | planned         | GPU vendor/VRAM, CPU, RAM                |
| Engine/model recommendation | planned         | Optimal engine + model based on hardware |
| First-launch wizard         | requires design | User-friendly setup flow                 |

</details>

<details>
<summary><b>Post-MVP — UX / Frontend</b></summary>

| Feature               | Status          | Details                                           |
| --------------------- | --------------- | ------------------------------------------------- |
| Overlay indicator     | requires design | Transparent always-on-top window, click-through   |
| Circular waveform     | requires design | Compact dictation indicator                       |
| Subtle idle indicator | requires design | Minimal, like OpenWhispr                          |
| Settings page         | requires design | STT engine, input mode, history, hotkey, models   |
| History UI            | requires design | Command/dictation log with configurable retention |

</details>

<details>
<summary><b>Post-MVP — Banners / Guides</b></summary>

| Banner                | Platform | Details                         |
| --------------------- | -------- | ------------------------------- |
| Admin elevation       | Windows  | UIPI warning if not elevated    |
| Model guide           | All      | First-launch model selection    |
| Cyrillic path warning | Windows  | Warning + autofix               |
| Wayland limitations   | Linux    | Clipboard+paste fallback notice |

</details>

<details>
<summary><b>Future — macOS</b></summary>

| Feature                    | Details                                                         |
| -------------------------- | --------------------------------------------------------------- |
| macOS support              | Accessibility + Microphone permissions, Notarization, Metal GPU |
| Apple Silicon optimization | Metal backend for ONNX Runtime and whisper.cpp                  |

</details>
