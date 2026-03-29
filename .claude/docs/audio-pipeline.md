# Audio Pipeline Architecture

```
Микрофон (cpal, always-on)
    │
    ▼
Silero VAD v6 (~1.8MB ONNX, always-on, <1% CPU)
    │ речь обнаружена
    ▼
Буфер 2-3 сек аудио (silence timeout: 300ms standby / 800ms dictation)
    │
    ▼ (в Standby/AwaitingSubcommand mode)
Google speech-embedding (mel ONNX 1MB + emb ONNX 1.3MB)
    → frame-level DTW matching (dtw_rs)
    │
    ├─ "Крео, приём" → AwaitingSubcommand mode (10s timeout → Standby)
    │   → Tier 1: DTW frame-level matching (✅ implemented, <50ms)
    │   → Tier 2: Vosk grammar mode (planned, <100ms)
    │   → Tier 3: Qwen3 1.7B + GBNF (planned, 0.5-2s)
    │
    ├─ "Крео, вписывай" → Dictation mode
    │   → STT engine (parakeet-rs primary, whisper-rs fallback)
    │   → enigo → ввод текста в активное приложение
    │
    ├─ "Крео, готово" → Stop dictation → inject text → Standby
    ├─ "Крео, отмена" → Cancel dictation → Standby (без injection)
    │
    └─ Нет совпадения → discard, возврат к VAD listening
```

## Silero VAD v6 — деталь имплементации

Каждый 512-sample чанк **ОБЯЗАТЕЛЬНО** должен быть prepended 64 samples контекста от предыдущего чанка. Итоговый input tensor: shape `(1, 576)`, не `(1, 512)`. Без контекста модель всегда возвращает ~0.0005 (не детектирует речь).

Tensor interface (v6):

- Inputs: `input` (1, 576), `state` (2, 1, 128), `sr` scalar i64
- Outputs: `output` (1, 1) probability, `stateN` (2, 1, 128)
- Context: последние 64 samples текущего чанка → prepend к следующему
- Reset: state и context обнуляются

## Модели

| Модель                    | Назначение                                     | Размер      | Формат      |
| ------------------------- | ---------------------------------------------- | ----------- | ----------- |
| Silero VAD v6             | Voice Activity Detection (always-on)           | ~1.8 MB     | ONNX        |
| Mel spectrogram           | Wake word preprocessing                        | ~1 MB       | ONNX        |
| Speech embedding          | Wake word 96-dim embeddings                    | ~1.3 MB     | ONNX        |
| Whisper base              | Fallback STT, текущий placeholder              | ~150 MB     | GGML        |
| Parakeet TDT 0.6B v3 INT8 | Main STT (ONNX Runtime)                        | ~640 MB     | ONNX        |
| Whisper models via ct2rs  | Отложен (оптимизация пограничных конфигураций) | 500MB-1.5GB | CTranslate2 |

## Hardware Acceleration Coverage

| Конфигурация                     | Текущее ускорение                                             | Выжат максимум?      | Что нужно для максимума                                                                                                                                                                              |
| -------------------------------- | ------------------------------------------------------------- | -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| NVIDIA GPU (любая)               | ✅ CUDA через ONNX Runtime (Parakeet) + whisper.cpp (Whisper) | ✅ Да                | —                                                                                                                                                                                                    |
| AMD GPU Windows                  | ✅ DirectML через ONNX Runtime (Parakeet)                     | ✅ Да                | —                                                                                                                                                                                                    |
| AMD GPU Linux                    | ✅ ROCm/Vulkan через ONNX Runtime                             | ✅ Да                | —                                                                                                                                                                                                    |
| Intel iGPU                       | ✅ DirectML/OpenVINO через ONNX Runtime                       | ✅ Да                | —                                                                                                                                                                                                    |
| Apple Silicon                    | ✅ CoreML (Parakeet) + Metal (whisper.cpp)                    | ✅ Да                | —                                                                                                                                                                                                    |
| AMD Ryzen CPU (без GPU)          | ✅ ONNX Runtime CPU (AVX2)                                    | ⚠️ ~90%              | Нет Zen-оптимизированного BLAS в экосистеме. AOCL существует, но ни один STT engine не использует. Прирост ~10-20% потенциально.                                                                     |
| Intel Core 11+ gen CPU (без GPU) | ✅ ONNX Runtime CPU (AVX-512)                                 | ⚠️ ~70-80%           | CTranslate2 + Intel MKL даёт 2-5x ускорение Whisper на Intel CPU через специализированные матричные инструкции (AVX-512, AMX). Интеграция через ct2rs — отложена, не критична пока Parakeet primary. |
| Intel Core 8-10 gen CPU          | ✅ ONNX Runtime CPU (AVX2)                                    | ⚠️ ~80%              | MKL даёт умеренный прирост через AVX2 оптимизации.                                                                                                                                                   |
| Старые CPU (<8th gen)            | ✅ ONNX Runtime CPU                                           | ✅ Потолок достигнут | Нет аппаратных инструкций для дальнейшего ускорения.                                                                                                                                                 |

### Потенциал для будущей оптимизации (ct2rs)

**Что:** CTranslate2 через ct2rs — ускоренный inference Whisper моделей с Intel MKL.
**Кого оптимизирует:** Intel Core 8+ gen на CPU-only (без GPU). ~2-5x ускорение Whisper.
**Кого НЕ затрагивает:** AMD Ryzen (MKL не оптимизирован для AMD), любая конфигурация с GPU (GPU и так быстрее).
**Статус:** Отложен. Parakeet через ONNX Runtime на CPU уже 30x real-time — быстрее чем ct2rs + Whisper + MKL. ct2rs актуален только для Whisper fallback (99 languages) на Intel CPU без GPU.
**Когда делать:** Когда все основные фичи реализованы и нужна финальная оптимизация CPU performance для Intel-only пользователей.

**Минусы внедрения ct2rs (причины отложения):**

- **ndarray version conflict:** ct2rs использует ndarray 0.16, наш проект — 0.17. Cargo скомпилирует обе версии, но типы несовместимы между ними + bloat бинарника.
- **OpenMP runtime conflict risk:** CTranslate2 с Intel MKL линкует Intel OpenMP (iomp5), ONNX Runtime может линковать MSVC OpenMP (vcomp). Два OpenMP runtime в одном процессе = потенциальные deadlocks. Решаемо через static linking, но добавляет complexity.
- **Context carry-over не прокинут:** ct2rs high-level `Whisper::generate()` не принимает prompt tokens — строит prompt внутри (только language + task). Для cross-segment coherence нужен форк или low-level API.
- **Тяжёлый build:** CMake компилирует CTranslate2 из C++ source (5-15 минут first build). Третий нативный build chain (whisper.cpp + ONNX Runtime + CTranslate2).
- **Отдельный формат моделей:** CTranslate2 модели ≠ GGML модели. Нужна конвертация через Python (`ct2-transformers-converter`). Пользователь не может переиспользовать whisper-rs модели.
- **Бинарник:** +30-80MB к размеру приложения (CTranslate2 runtime).

## Auto-Configuration (первый запуск)

1. Определяем GPU (vendor, VRAM), CPU, RAM
2. Подбираем оптимальный движок + модель + квантизацию
3. Показываем пользователю в понятном виде (без технических деталей)
4. Пользователь может переопределить выбор
