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
    ├─ "Крео, приём" → AwaitingSubcommand mode
    │   → DTW / Vosk grammar / Qwen3 LLM (tiered cascade)
    │
    ├─ "Крео, вписывай" → Dictation mode
    │   → STT engine (ct2rs или parakeet-rs, user-selectable)
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

| Модель                    | Назначение                               | Размер      | Формат      |
| ------------------------- | ---------------------------------------- | ----------- | ----------- |
| Silero VAD v6             | Voice Activity Detection (always-on)     | ~1.8 MB     | ONNX        |
| Mel spectrogram           | Wake word preprocessing                  | ~1 MB       | ONNX        |
| Speech embedding          | Wake word 96-dim embeddings              | ~1.3 MB     | ONNX        |
| Whisper base              | Dictation placeholder (текущий)          | ~150 MB     | GGML        |
| Parakeet TDT 0.6B v3 INT8 | Main STT (ONNX Runtime, user-selectable) | ~640 MB     | ONNX        |
| Whisper models via ct2rs  | Main STT (CTranslate2, user-selectable)  | 500MB-1.5GB | CTranslate2 |

## GPU Compatibility

| GPU                    | CTranslate2 (ct2rs) | Parakeet (ONNX Runtime) |
| ---------------------- | ------------------- | ----------------------- |
| NVIDIA (CUDA)          | ✓                   | ✓                       |
| AMD (Windows/DirectML) | ✗                   | ✓                       |
| AMD (Linux/ROCm)       | ✗                   | ✓                       |
| Intel iGPU             | ✗                   | ✓ (DirectML/OpenVINO)   |
| CPU only               | ✓ (int8)            | ✓                       |

## Auto-Configuration (первый запуск)

1. Определяем GPU (vendor, VRAM), CPU, RAM
2. Подбираем оптимальный движок + модель + квантизацию
3. Показываем пользователю в понятном виде (без технических деталей)
4. Пользователь может переопределить выбор
