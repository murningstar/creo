# Audio Pipeline Architecture

```
Микрофон (cpal, always-on)
    │
    ▼
Silero VAD (~1.8MB ONNX, always-on, <1% CPU)
    │ речь обнаружена
    ▼
Буфер 2-3 сек аудио
    │
    ▼
whisper-rs tiny (~75MB GGML, CPU burst ~0.2s)
    │ fuzzy match "Крео, приём/вписывай/готово"
    │
    ├─ "Крео, приём" → Tauri IPC → frontend: command mode
    │   → Запуск Kando (MVP)
    │
    ├─ "Крео, вписывай" → Tauri IPC → frontend: dictation mode
    │   → Прогрев основного STT (ct2rs или parakeet-rs)
    │   → Непрерывная транскрипция → enigo → ввод текста
    │
    ├─ "Крео, готово" → Tauri IPC → frontend: stop dictation
    │   → Остановка STT, финализация текста
    │
    └─ Нет совпадения → discard, возврат к VAD listening
```

## Silero VAD v5 — критичная деталь имплементации

Каждый 512-sample чанк **ОБЯЗАТЕЛЬНО** должен быть prepended 64 samples контекста от предыдущего чанка. Итоговый input tensor: shape `(1, 576)`, не `(1, 512)`. Без контекста модель всегда возвращает ~0.0005 (не детектирует речь).

Источник: `snakers4/silero-vad/src/silero_vad/utils_vad.py` — `OnnxWrapper.__call__()`.
Рабочая Rust-реализация: `sheldonix/silero-vad-rust`.
Сломанная Rust-реализация (без контекста): `nkeenan38/voice_activity_detector`.

Tensor interface (v5):

- Inputs: `input` (1, 576), `state` (2, 1, 128), `sr` scalar i64
- Outputs: `output` (1, 1) probability, `stateN` (2, 1, 128)
- Context: последние 64 samples текущего чанка → prepend к следующему
- Reset: state и context обнуляются

## Модели (скачиваются при первом запуске)

| Модель                       | Назначение               | Размер      | Формат      |
| ---------------------------- | ------------------------ | ----------- | ----------- |
| Silero VAD                   | Voice Activity Detection | ~1.8 MB     | ONNX        |
| Whisper tiny                 | Wake word detection      | ~75 MB      | GGML        |
| Whisper small/large-v3-turbo | Main STT (CTranslate2)   | 500MB-1.5GB | CTranslate2 |
| Parakeet TDT 0.6B            | Main STT (альтернатива)  | ~600 MB     | ONNX        |

## GPU Compatibility

| GPU                    | CTranslate2 (ct2rs) | whisper.cpp (whisper-rs) | Parakeet (ONNX Runtime) |
| ---------------------- | ------------------- | ------------------------ | ----------------------- |
| NVIDIA (CUDA)          | ✓                   | ✓                        | ✓                       |
| AMD (Windows/DirectML) | ✗                   | ✓ (Vulkan)               | ✓                       |
| AMD (Linux/ROCm)       | ✗                   | ✓ (Vulkan)               | ✓                       |
| Intel iGPU             | ✗                   | ✓ (Vulkan)               | ✓ (DirectML/OpenVINO)   |
| CPU only               | ✓ (int8)            | ✓                        | ✓                       |

## Auto-Configuration (первый запуск)

1. Определяем GPU (vendor, VRAM), CPU, RAM
2. Подбираем оптимальный движок + модель + квантизацию
3. Показываем пользователю в понятном виде (без технических деталей)
4. Пользователь может переопределить выбор
