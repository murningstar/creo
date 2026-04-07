# Creo Evolution Plan: Audio Pipeline & Voice Command Architecture

> Документ создан 2026-03-26, обновлён 2026-03-26 по результатам 13+ research agents.
> Это единый источник правды для архитектурных решений audio pipeline.
> **Сверяться:** при любых изменениях в audio pipeline, при анализе текущего состояния точек соприкосновения технологий, при неполадках (плохое определение wake words, плохая транскрипция, false positives).

---

## Принципы выбора решений

- **Quality > model size.** Разница в 50-300MB ничтожна. 500MB RAM — не проблема. НЕ стремиться к легковесным 2-10MB если тяжелее = качественнее.
- **Реальные блокеры (приоритет):** 1) UX friction (сколько шагов до работающей системы; 10-15 записей OK, 30+ — блокер) 2) CPU idle consumption (>10% в простое — плохо) 3) Multilingual обязательно 4) CPU-only fallback обязателен (GPU = бонус)
- **Параметрические команды** создаются пользователем через UI (phrase template с плейсхолдерами). System prompt и GBNF grammar генерируются динамически из пользовательских шаблонов.

---

## Финальная архитектура

```
Standby mode:
  Mic → cpal → Silero VAD → speech buffer
    → Google 96-dim embeddings → DTW frame-level (now) / conv-attention (target)
    → Wake word matched:
        "вписывай" → Dictation mode
        "приём" → AwaitingSubcommand mode
        "готово" / "отмена" → (handled separately in Dictation mode)

Dictation mode:
  Mic → Silero VAD (800ms silence threshold) → 500ms audio overlap
    → Parakeet TDT 0.6B v3 (primary) / Whisper base (fallback, current)
    → text with punctuation → inject into active app
  Stop/Cancel detection:
    → Embedding DTW match on each segment + text verification gate on match

AwaitingSubcommand mode:
  Mic → Silero VAD → speech buffer
    → Tier 1: Embedding DTW (fixed subcommands, <50ms, audio-level)
    → Tier 2: Vosk grammar mode (known text subcommands, ~50ms, text-level)
    → Tier 3: Qwen3 1.7B + GBNF (parametric + free-form, 1-2s)
```

### Почему 3 tiers вместо 4

Исследования показали: template/regex (бывший Tier 3) и MiniLM (бывший Tier 3) **не стоят complexity**. Qwen3 0.6B с GBNF constrained decoding справляется и с простыми, и со сложными командами одинаково, при приемлемых 0.5-1s. Template matching хрупкое и не масштабируется. MiniLM — sentence encoder без native slot extraction. LLM с GBNF — universal parser.

### DTW vs Vosk — разные задачи, разные этапы flow

|                                | DTW (audio embeddings)                                                                                                      | Vosk (grammar-constrained STT)             |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| **Когда**                      | Wake words (Standby), stop/cancel (Dictation)                                                                               | Subcommands (AwaitingSubcommand)           |
| **Как пользователь добавляет** | Записывает голосом 3-15 сэмплов                                                                                             | Вводит текстом название команды            |
| **Language support**           | Language-agnostic (audio-level)                                                                                             | Нужна модель per-language                  |
| **Масштабируемость**           | 3-10 commands (embedding space crowding)                                                                                    | 10-50+ commands (grammar scales trivially) |
| **Зачем оба**                  | Wake words ДОЛЖНЫ работать на audio-level (нет STT в Standby). Subcommands могут позволить STT (уже знаем что ждём команду) |

---

## Финальный выбор моделей

| Роль                           | Модель                                 | Size           | CPU perf                                  | Почему именно эта                                                                                                                                                                                    |
| ------------------------------ | -------------------------------------- | -------------- | ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| VAD (always-on)                | **Silero VAD v6**                      | 1.8MB          | <1% idle                                  | Best-in-class, уже интегрирована, ONNX                                                                                                                                                               |
| Wake word embedding            | **Google speech-embedding CNN 96-dim** | 2.3MB          | <1% idle                                  | Обучена на keyword discrimination (правильный objective). ECAPA-TDNN — ОТВЕРГНУТА (speaker verification, подавляет phonetic info)                                                                    |
| Wake word classifier (target)  | **livekit-wakeword conv-attention**    | ~100KB/command | <1% idle                                  | 100x fewer FP (0.08 FPPH), 86.1% recall. Pre-trained offline + on-device fine-tune (10-30s). ⛔ TTS pipeline на устройстве инвалидирован                                                             |
| Dictation STT (primary)        | **Parakeet TDT 0.6B v3 INT8**          | ~671MB         | **RTF 0.033** (30x real-time on i7-12700) | 5.51% WER Russian, native punctuation/capitalization, auto language detection (25 EU langs). Быстрее Whisper small на CPU при dramatically лучшем quality. ONNX Runtime: CUDA/DirectML/CPU — все GPU |
| Dictation STT (fallback)       | **Whisper base via whisper-rs**        | ~150MB         | Slower than Parakeet                      | whisper.cpp, GGML. Текущий placeholder, остаётся как fallback                                                                                                                                        |
| Dictation STT (отложен)        | **Whisper models via ct2rs**           | 500MB-1.5GB    | Varies by model                           | CTranslate2 runtime. Отложен до реализации всех основных фич; для оптимизации пограничных конфигураций (Intel CPU-only). Блокеры в audio-pipeline.md                                                 |
| Subcommand recognition         | **Vosk + grammar**                     | ~50MB/lang     | burst only                                | 0 записей от пользователя, `[unk]` rejection, streaming, Apache 2.0, Rust bindings (vosk-rs)                                                                                                         |
| Command NLU (primary)          | **Qwen3 1.7B Q4_K_M**                  | ~1GB           | 1-2s burst                                | **0.960** tool-calling benchmark, 119 languages, GBNF JSON output. Performs on par with Qwen2.5-3B                                                                                                   |
| Command NLU (weak HW fallback) | **Qwen3 0.6B Q4_K_M**                  | ~400MB         | 0.5-0.75s                                 | **0.880** tool-calling (бьёт FunctionGemma 270M: 0.640, Phi-4-mini 3.8B: 0.780, Gemma 3 1B: 0.550)                                                                                                   |

---

## Архитектурные описания кандидатов

> Суть каждой технологии: что это, как работает, для пользователя/разработчика.

### Wake Word Detection: два отдельных компонента

Wake word detection — это **два компонента** в цепочке, не один:

```
Аудио → [1. Embedding модель] → отпечаток (вектор чисел) → [2. Сравниватель] → "совпало / не совпало"
```

**Компонент 1: Embedding модель** — превращает звук в компактный "отпечаток" (вектор чисел). Качество этого отпечатка определяет, какие звуки модель МОЖЕТ различить в принципе. Если два разных слова дают одинаковый отпечаток — никакой сравниватель их не разделит. Embedding модель обучается один раз и не меняется при добавлении новых команд.

**Компонент 2: Сравниватель** — решает, "совпал ли отпечаток входящего звука с сохранённым эталоном". Может быть алгоритмом (cosine similarity, DTW) или обученной нейросетью (conv-attention classifier). Качество сравнивателя определяет, насколько точно он использует информацию из отпечатков.

**Путь улучшения идёт по обоим компонентам независимо:**

- Лучший embedding = модель "видит" больше различий между звуками
- Лучший сравниватель = точнее решает "совпало / не совпало" на тех же отпечатках

---

**Embedding: Google speech-embedding CNN 96-dim (текущий) — ✅ VERIFIED best deployable choice (March 2026)**

Два ONNX: mel spectrogram → 96-dim embedding. CNN обучена на ~200M YouTube клипов с 5000 keywords — "звуковой отпечаток" конкретной фразы. Training objective: keyword discrimination (различать ЧТО сказано). English-biased (обучена на английском YouTube) — для не-английских фраз пространство менее дискриминативно.

**Верификация (hard data):** SSL модели (Wav2Vec 2.0 + Sub-center ArcFace) дают ЛУЧШИЕ embeddings для KWS (82.2% at 1% FAR vs no comparable published number for Google 96-dim). Но:

- Полные SSL модели (95M-300M params, 360MB-1.2GB) слишком тяжёлые для real-time CPU inference
- Distilled student (EdgeSpot, 128K params, 82.0% at 1% FAR) **не опубликован** как ONNX, English-only MSWC
- ECAPA-TDNN **подтверждённо хуже для KWS:** 5.58% FRR vs Conformer 1.63% (3.4x gap, QbyE KWS paper 2024)
- Google 96-dim — единственная purpose-built KWS embedding с production validation (openWakeWord, livekit-wakeword, local-wake) и ONNX availability

**Вывод:** bottleneck — сравниватель (cosine→DTW→conv-attention), не embedding. livekit-wakeword достигает 100x fewer FP на тех же Google embeddings через лучший classifier. Менять embedding имеет смысл только когда появится multilingual distilled KWS embedding (EdgeSpot-style) в ONNX.

**🔄 ПЕРИОДИЧЕСКИЙ МОНИТОРИНГ:** При каждом крупном рефакторинге audio pipeline или раз в ~2-3 месяца — запускать исследование: "появился ли в open-source multilingual KWS embedding лучше Google 96-dim? Опубликован ли EdgeSpot student? Новые модели на MSWC/GSC бенчмарках?" Предметная область активно развивается (EdgeSpot Jan 2026, MT-HuBERT Nov 2025) — breakthrough может появиться в любой момент.

**Сравниватель (текущий): DTW (Dynamic Time Warping) — алгоритм, не модель**
Сравнивает две последовательности frame-by-frame, находя оптимальное "наложение". Сохраняет порядок звуков, который mean-embedding теряет. "Крео вписывай" отличается от "рекомендую" потому что слоги идут в другом порядке. Не обучаемый — работает на чистой математике (dynamic programming). Crate: `dtw_rs` (v0.10.0, MIT, zero deps, generic T, custom distance closure, Sakoe-Chiba band).

**DTW параметры (calibrated from production systems + research):**
| Параметр | Значение | Источник / обоснование |
|----------|----------|----------------------|
| Distance threshold | 0.20 | Calibrated: true matches 0.05-0.15, false positives 0.24+. local-wake uses 0.1 as example. Snips/Raven: 0.22 for MFCC (different scale) |
| Sakoe-Chiba band | 3 | ~25% of typical sequence (8-12 frames). Raven/Rustpotter use 5 but for 50+ frame MFCC sequences (~10%) |
| Normalization | total / (len_a + len_b) | **Standard across ALL production KWS:** Raven, Rustpotter, local-wake, Snips. Более стабильный чем path-length (не зависит от alignment quality) |
| Min input frames | 7 (~600ms) | Выше 50% от типичного template (10-12 frames). Отсекает короткие шумы |
| Score aggregation | min distance | Best single match across reference samples. Standard approach (Raven, local-wake) |
| Cosine distance | 1.0 - cos_sim | Per-frame, bounded [0, 2]. Normalized DTW bounded ≈ [0, 1] |

**Сравниватель (целевой): livekit-wakeword conv-attention classifier — обученная нейросеть**
Надстройка над теми же embeddings (не заменяет embedding модель). Conv1D → Multi-head Attention → Sigmoid. В отличие от DTW, это обученная модель — она видела тысячи примеров "да, это фраза" и "нет, это не фраза", и научилась РАЗЛИЧАТЬ. 100x fewer false positives (0.08 FPPH vs 8.50 openWakeWord) на тех же embedding'ах. Rust crate Apache-2.0.

**Два шага одного подхода:**

1. **Мы (offline):** тренируем base classifier через полный TTS pipeline (VITS/Piper генерирует 25K синтетических клипов + adversarial negatives, 3-phase adaptive training, export ONNX ~100KB). Занимает 1-2 часа на GPU. Поставляется с приложением.
2. **Пользователь (on-device):** записывает 3-5 сэмплов своим голосом → fine-tune pre-trained classifier + augmentation → **10-30 секунд на CPU**. Модель адаптируется к голосу пользователя.

**⛔ TTS training pipeline на устройстве пользователя — ИНВАЛИДИРОВАН.** Полный pipeline занимает 7-13 часов на CPU (bottleneck: VITS neural TTS генерация 25K клипов). Требует Python + PyTorch. Неприемлемо для UX. Только pre-trained models + on-device fine-tuning.

**Для пользователя:** записывает 3-5 примеров, ждёт 10-30 секунд. Идентично текущему enrollment, но classifier вместо cosine similarity.
**Для нас:** поставляем pre-trained ONNX classifiers для base commands. Fine-tuning pipeline можно реализовать в Rust (модель ~6K-200K params, augmentation на audio-level). `livekit-wakeword` Rust crate для inference.

### ОТВЕРГНУТЫЕ кандидаты для wake word (с обоснованием)

**ECAPA-TDNN 192-dim — ОТВЕРГНУТА**
Speaker verification модель (КТО говорит, не ЧТО сказано). Training objective: discriminate 5994 speakers on VoxCeleb2. Активно подавляет phonetic information. Использование для keyword detection сделало бы quality ХУЖЕ, несмотря на 192 dims vs 96. Может быть полезна как secondary speaker filter ("отвечать только на МОЙ голос"), но не как primary keyword discriminator.

**Sherpa-ONNX KWS — ОТЛОЖЕНА**
Dedicated keyword spotting, Zipformer ~3MB, streaming. Но нет multilingual models (только Chinese + English). Мониторим — когда появятся multilingual, пересмотрим.

**Constrained Whisper GBNF — ОТВЕРГНУТА для subcommands**
Grammar support в whisper.cpp buggy: results outside grammar, partial word matching errors, grammars silently ignored. Не тестировано с Cyrillic. Слишком ненадёжно для production.

### Subcommand Recognition

**Vosk grammar mode (выбран)**
Kaldi-based ASR с FST (Finite State Transducer). Grammar mode: передаёшь JSON array допустимых фраз → Vosk распознаёт только из этого списка, иначе `[unk]`. Streaming, ~50ms, ~50MB/lang, Apache 2.0. **Для пользователя:** вводит текст названия команды в UI, ноль записей. **Для нас:** `vosk-rs` Rust bindings, `Recognizer::new_with_grammar(model, rate, grammar_json)`.

**Почему Vosk, а не DTW для subcommands:**

- DTW: пользователь записывает 3-15 сэмплов per command. При 50 subcommands = 150-750 записей → UX blocker.
- Vosk: пользователь вводит текст. 50 subcommands = 50 строк текста → приемлемо.
- DTW: embedding space crowding при 50 commands. Vosk: grammar scales trivially.

### Command NLU

**Qwen3 1.7B Q4_K_M (primary) / Qwen3 0.6B Q4_K_M (fallback)**

Small LLM с native function calling + GBNF grammar-constrained JSON output через llama.cpp.

Benchmarks (BFCL tool-calling):
| Model | Score | Size Q4 | CPU speed |
|-------|-------|---------|-----------|
| **Qwen3 1.7B** | **0.960** | ~1GB | 15-30 tok/s, 1-2s for response |
| **Qwen3 0.6B** | **0.880** | ~400MB | 40-60 tok/s, 0.5-0.75s |
| FunctionGemma 270M | 0.640 | ~150MB | Fast but unreliable |
| Phi-4-mini 3.8B | 0.780 | ~2GB | Too slow on CPU |
| Gemma 3 1B | 0.550 | ~700MB | Worse than Qwen3 0.6B |
| LFM2.5 1.2B | 0.920 | <1GB | 239 tok/s AMD CPU (worth watching) |

**GBNF constrained decoding — key enabler.** Ограничивает output до valid JSON matching нашей intent schema. Eliminates hallucinations structurally. Disproportionately benefits small models — модель "выбирает" из предопределённых вариантов, а не генерирует свободный текст.

**Для пользователя:** два пути создания команд:

1. **Простая подкоманда** (Vosk tier): вводит текст фразы в UI → Vosk распознаёт из списка. Без плейсхолдеров.
2. **Параметрическая команда** (Qwen3 tier): вводит phrase template с плейсхолдерами в UI, например `"переведи слово {word} с {source_lang} на {target_lang}"`. Привязывает action. Qwen3 парсит свободную речь → заполняет slots.

Пользователь сам создаёт и простые, и параметрические команды через UI. Это НЕ блокер — template с плейсхолдерами интуитивен.

**Для нас:**

- `llama-cpp-2` Rust crate, GGUF models, GBNF grammar file
- System prompt **генерируется динамически** из пользовательских templates: каждый template с плейсхолдерами → JSON function schema в system prompt
- GBNF grammar тоже генерируется из пользовательских slot definitions
- Model loaded on first AwaitingSubcommand → kept warm 30s → unloaded
- UI: форма создания команды с полем template и кнопкой "add placeholder" для каждого slot

### ОТВЕРГНУТЫЕ NLU кандидаты

**FunctionGemma 270M — ОТВЕРГНУТА**
58% zero-shot BFCL (vs Qwen3 0.6B: 68%). English-only fine-tuning. Google docs: "fine-tune for non-English." Добавление нового intent = retrain. Размер (150MB) не advantage когда 400MB модель бьёт её по quality.

**Template/regex — ОТВЕРГНУТА**
Хрупкое, ломается на вариации ("как будет X по-английски?"). Не масштабируется. 1-2s latency LLM приемлема → regex не даёт meaningful benefit.

**MiniLM ONNX — ОТВЕРГНУТА**
Sentence encoder, не generative. Может classify intent, но НЕ может extract arbitrary slots. Нужен separate slot extractor → double complexity. Qwen3 делает оба одновременно.

**JointBERT/JointIDSF — ОТВЕРГНУТА**
Requires training data per intent schema, doesn't generalize to new intents without retraining. Qwen3 zero-shot + schema change = instant new capabilities.

### Dictation STT

**Parakeet TDT 0.6B v3 INT8 (primary STT)**
FastConformer encoder + Token-and-Duration Transducer. Non-autoregressive — predicts token AND skip in one step → fundamentally faster than Whisper.

Key metrics:

- Russian FLEURS WER: **5.51%**, CoVoST: **3.00%**
- CPU RTF: **0.033** on i7-12700KF (30x real-time), **0.059** on old i7-4790 (17x real-time)
- **Outperforms faster-whisper on RTX 3070 Ti by 2.25x — on CPU alone**
- INT8: zero accuracy loss vs FP32
- 25 EU languages, auto detection
- Native punctuation + capitalization
- License: CC-BY-4.0 (commercial OK)

**ВАЖНО: Parakeet TDT НЕ streaming через ONNX** (sherpa-onnx issue #2918). ONNX export = offline only. True streaming requires NeMo Python runtime. Но при RTF 0.033, **chunked pseudo-streaming** работает отлично: VAD → 2-5s chunk → Parakeet (~50-150ms) → text appears.

Rust: `parakeet-rs` v0.3.4 (MIT/Apache-2.0), DirectML, CUDA, CPU fallback.

**Приоритет STT движков:**

- **Parakeet TDT 0.6B v3** (parakeet-rs) — **primary STT.** ONNX Runtime: CUDA/DirectML/CPU — все GPU-вендоры. 25 EU languages, native punctuation.
- **Whisper base** (whisper-rs) — **fallback STT.** Текущий placeholder, остаётся как fallback после интеграции Parakeet.
- **Whisper via ct2rs** (CTranslate2) — **отложен.** Актуален для оптимизации пограничных конфигураций (Intel CPU-only, 99 languages). Возвращаемся только после реализации всех основных фич. Блокеры и обоснование в audio-pipeline.md. Замечено: RTF ~1.10 для turbo на i5-12450H (int8, CPU). **distil-large-v3 исключена** — быстрее (RTF ~0.94), но непригодна для мультиязычной транскрипции (выдаёт английский вместо русского при `language="ru"`).
- Wake word detection использует отдельные ONNX модели (mel + embedding), НЕ STT движок.

**Архитектурное различие Whisper vs Parakeet:**

- **Whisper:** batch model. Context carry-over через explicit prompt_tokens — нужно вручную передавать. Risk: hallucination loops.
- **Parakeet:** transducer, designed for chunked inference с overlapping attention windows. Context carry-over встроен в архитектуру. Нативная пунктуация.

### Dictation Segmentation

**Текущие root causes проблем:**

1. `set_no_context(true)` в stt.rs (WhisperEngine) — **явно отключает** context carry-over
2. `VAD_SILENCE_TIMEOUT_MS = 300` — слишком агрессивно для dictation
3. Нет audio overlap — жёсткие VAD-разрывы
4. Whisper tiny — неадекватен для не-английского
5. Нет hallucination filtering

**Наблюдаемые симптомы:**

```
Надиктовано: "Раз раз раз, это харбасс", затем числа 1-41.
Результат: "Рас-рас-рас, это Хардбас." / "и и цить" / "Роб 6" / "Всем."
```

**Quick fixes (на текущем Whisper, часы работы):**

| Fix                                        | Что                                                                                  | Влияние                                   |
| ------------------------------------------ | ------------------------------------------------------------------------------------ | ----------------------------------------- |
| `set_no_context(false)` + `initial_prompt` | Prompt window 224 tokens. Token-level carry-over recommended                         | **Высокое** — cross-segment coherence     |
| Audio overlap 500ms pre-buffer             | `pre_buffer: Vec<f32>` последних 8000 samples, prepend к speech_buffer               | **Высокое** — нет обрубков слов           |
| Dictation silence threshold 800ms          | Mode-dependent: Standby 300ms, Dictation 800-1000ms                                  | Среднее                                   |
| Hallucination filtering                    | compression_ratio > 2.4 → discard. no_speech_prob > 0.6 → discard. Min segment 500ms | Среднее                                   |
| Segment merging                            | Склеивать соседние сегменты с <1s паузой в один chunk (до 15-20s)                    | Высокое — fewer boundaries = fewer errors |

**Context carry-over в Whisper (3 способа через whisper-rs):**

1. Token-level (recommended): `state.full_get_token_data()` → `set_tokens()` (как в whisper.cpp stream.cpp)
2. Text-level (проще): `set_initial_prompt()` с текстом предыдущего сегмента
3. Reuse WhisperState с `no_context=false` (авто carry-over, но наш код создаёт state заново)

**Risk: hallucination loops** (issue #1017). Mitigation: `n_max_text_ctx=64`, `single_segment=true`, entropy threshold 2.8.

**С Parakeet:** segmentation problem уменьшается (лучшая модель → меньше ошибок на boundaries), но НЕ исчезает (ONNX = offline, всё равно VAD-segmented). Quick fixes (overlap, silence threshold, segment merging) **переносятся на любой движок**.

### Stop/Cancel Detection During Dictation

**Ключевой инсайт:** VAD уже изолирует stop-команды в отдельные сегменты. Пользователь делает natural pause перед "Крео, готово" — cognitive intent switch вызывает prosodic break → VAD сегментирует → embedding match работает на isolated utterance.

**Проблемный edge case:** пользователь диктует слово "готово" как часть текста. Приходит как isolated short segment → embedding match сработает.

**Решение:** тот же embedding DTW + lightweight text verification gate. Stage 2 только когда Stage 1 matched → Whisper tiny transcribes (~200ms) → проверяет содержит ли текст trigger phrase. Fires rarely (раз в часы dictation), latency negligible.

---

## Reference architectures

### Amazon Alexa (production, not for copying)

- Stage 1: DNN-HMM, 4-layer feedforward, 20-dim LFBE
- Stage 2: Lightweight NN на 67-dim monophone features. **67% FAR reduction**
- Stage 3: Cloud CRA. **53-55% additional FAR reduction**
- Инсайт: каждый stage — specialized acoustic classifier, НЕ general STT

### OVOS / Mycroft (open-source intent cascade)

```
padatious_high → adapt_high → padatious_medium → adapt_low → Model2Vec → LLM fallback
```

Быстрые keyword-matchers первыми, ML на fallback. Тот же принцип что наш Tier 1→2→3.

### Home Assistant Speech-to-Phrase

Kaldi + FST compiled from sentence templates. "Какую из миллиона фраз сказали?" за <1s на RPi. Fuzzy correction. 21 lang. Паттерн для Vosk grammar integration.

### Sensory Smart Wakewords (Jan 2026, commercial)

On-device wake word + on-device STT re-validation + micro-LLM/NLU. Единственный commercial кто uses STT verification, но proprietary embedded STT.

---

## Дополнительные техники false positive reduction (wake words)

- **Confidence ratio:** `best_match / second_best_match` — true wake word имеет высокий margin
- **Temporal smoothing:** Require N consecutive positive detections в time window
- **Negative class modeling:** Rolling buffer negative embeddings + rejection threshold
- **openWakeWord verifier models:** Logistic regression на embeddings, speaker-adapted

---

## Эволюционный путь (сводная)

```
Сейчас (April 2026)       Ближайшее будущее              Целевое состояние
────────────────────      ──────────────────              ─────────────────
DTW frame-level      →    (tuning)                  →    livekit-wakeword conv-attention
  (cosine fallback)
Whisper base dictation → Quick fixes (context,      →    Parakeet TDT 0.6B v3
                          overlap, filter)
DTW subcommands (T1) →    Vosk grammar (T2)         →    Vosk + Qwen3 1.7B + GBNF (T3)
Нет параметров       →    (skip)                    →    Qwen3 1.7B + GBNF
800ms dictation silence → Segment merging            →    Adaptive silence + Parakeet chunked
```

---

## Мониторинг (перспективные технологии)

| Технология                       | Что даёт                                         | Почему ждём                                |
| -------------------------------- | ------------------------------------------------ | ------------------------------------------ |
| **Sherpa-ONNX multilingual KWS** | Dedicated keyword spotting ~3MB, ONNX            | Нет multilingual models пока               |
| **Moonshine v2 multilingual**    | 5.8x faster than Whisper, better quality         | English only, non-commercial для non-EN    |
| **Qwen3.5 0.8B**                 | Successor to Qwen3, possibly better tool-calling | No benchmarks yet (released March 2, 2026) |
| **LFM2.5 1.2B**                  | 0.920 tool-calling, 239 tok/s AMD CPU            | New, less ecosystem                        |
| **Parakeet streaming ONNX**      | True streaming via ONNX (currently offline only) | sherpa-onnx issue #2918 open               |
| **Semantic endpointing**         | End-of-utterance from content, not silence       | Requires local LLM or cloud                |

---

## Ключевые findings из всех исследований

1. **Никто не использует general STT для wake word verification.** Amazon: specialized acoustic classifiers.
2. **Google speech-embedding — правильная модель** для keyword spotting. ECAPA-TDNN — WRONG model class (speaker verification).
3. **Frame-level DTW** — immediate fix. Conv-attention classifier — target (100x fewer FP).
4. **Whisper tiny + Parakeet:** Whisper tiny неадекватен для dictation. Parakeet TDT 0.6B v3 на CPU **быстрее** Whisper small при dramatically лучшем quality. Skip Whisper small.
5. **Parakeet НЕ streaming через ONNX.** Chunked pseudo-streaming при RTF 0.033 — OK для dictation.
6. **`set_no_context(true)` в нашем коде** — root cause потери контекста в dictation.
7. **Vosk grammar** — лучший choice для subcommands (0 recordings, `[unk]` rejection, scales to 50+).
8. **Qwen3 1.7B** — best-in-class tool-calling (0.960) для multilingual command parsing. FunctionGemma отвергнута (0.640).
9. **GBNF constrained decoding** — great equalizer для small LLMs. Structural hallucination prevention.
10. **Tiered cascade упрощён** до 3 tiers: embedding match → Vosk grammar → Qwen3 LLM.

---

## Research sources (cumulative)

### Wake word detection

- local-wake (st-matskevich) — 98.6% accuracy, same models, DTW
- livekit-wakeword — 100x fewer FPPH, conv-attention, Rust crate
- EdgeSpot (ICASSP 2026) — 128K params, 82% at 1% FAR, 10-shot SOTA
- openWakeWord — CC-BY-NC-SA, English-biased, dependency rot
- Few-Shot Open-Set KWS (2023) — triplet loss, 76% at 5% FAR
- Quantization-Based Score Calibration (Oct 2025) — robust threshold

### STT / Dictation

- Parakeet TDT 0.6B v3 — 5.51% WER Russian, RTF 0.033 CPU, CC-BY-4.0
- parakeet-rs v0.3.4 — Rust crate, DirectML/CUDA/CPU
- sherpa-onnx issue #2918 — Parakeet ONNX not streaming
- whisper.cpp stream.cpp — sliding window + prompt_tokens
- whisper-cpp-plus-rs — EnhancedWhisperVadProcessor, quality fallback
- Calm-Whisper (Interspeech 2025) — 80% hallucination reduction

### NLU / LLM

- Qwen3 Technical Report — 119 languages, tool-calling benchmarks
- MikeVeerman/tool-calling-benchmark — BFCL scores for small models
- FunctionGemma — 58% zero-shot, English-only fine-tuning
- Home Assistant Voice Agent LLM Benchmark — March 2026
- llama.cpp GBNF grammars — constrained decoding
- "Less is More" (2025) — tool filtering improves small model accuracy

### Subcommands

- Vosk grammar mode — FST-constrained recognition
- vosk-rs — Rust bindings (Bear-03/vosk-rs)
- Rhasspy fsticuffs — FST, millions of sentences, ms recognition
- Home Assistant Speech-to-Phrase — Kaldi + FST + fuzzy

### Architecture

- Amazon Alexa multi-stage — DNN-HMM → NN → CRA
- OVOS intent pipeline — cascading priority matchers
- Picovoice Rhino — Speech-to-Intent (proprietary reference)
- Sensory Smart Wakewords — on-device STT + micro-LLM
