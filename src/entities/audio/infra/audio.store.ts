import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { useSettingsStore } from '~/entities/settings';

import type {
    AudioErrorEvent,
    AudioStateEvent,
    ModelStatus,
    TranscriptionEvent,
    VadStateEvent,
    WakeCommandEvent,
} from '../model/types';
import { AudioMode } from '../model/types';

export const useAudioStore = defineStore('audio', () => {
    const _mode = ref<AudioMode>(AudioMode.Idle);
    const _isSpeech = ref(false);
    const _lastTranscription = ref('');
    const _error = ref<string | null>(null);
    const _unlisten = ref<UnlistenFn[]>([]);
    const _modelStatus = ref<ModelStatus | null>(null);

    const mode = readonly(_mode);
    const isSpeech = readonly(_isSpeech);
    const lastTranscription = readonly(_lastTranscription);
    const error = readonly(_error);
    const modelStatus = readonly(_modelStatus);

    const isListening = computed<boolean>(() => _mode.value === AudioMode.Listening);
    const isDictation = computed<boolean>(() => _mode.value === AudioMode.Dictation);
    const isProcessing = computed<boolean>(() => _mode.value === AudioMode.Processing);
    const isIdle = computed<boolean>(() => _mode.value === AudioMode.Idle);

    /** DEV ONLY: sets frontend mode without Tauri IPC — backend state will NOT change. */
    const __devSetMode = (newMode: AudioMode) => {
        _mode.value = newMode;
    };

    async function startListening() {
        _error.value = null;
        try {
            await invoke('start_listening');
        } catch (e) {
            _error.value = String(e);
        }
    }

    async function stopListening() {
        _error.value = null;
        try {
            await invoke('stop_listening');
        } catch (e) {
            _error.value = String(e);
        }
    }

    async function checkModels() {
        try {
            _modelStatus.value = await invoke<ModelStatus>('check_models');
        } catch (e) {
            _error.value = String(e);
        }
    }

    async function testCapture(): Promise<string | null> {
        _error.value = null;
        try {
            return await invoke<string>('test_capture');
        } catch (e) {
            _error.value = String(e);
            return null;
        }
    }

    async function setupEventListeners() {
        const listeners = await Promise.all([
            listen<AudioStateEvent>('audio-state-changed', event => {
                _mode.value = event.payload.mode;
            }),
            listen<VadStateEvent>('vad-state', event => {
                _isSpeech.value = event.payload.isSpeech;
            }),
            listen<TranscriptionEvent>('transcription', async event => {
                _lastTranscription.value = event.payload.text;

                // Inject text into active app when dictation produces final result
                if (event.payload.isFinal && _mode.value === AudioMode.Dictation) {
                    try {
                        const settings = useSettingsStore();
                        await invoke('inject_text', {
                            text: event.payload.text,
                            method: settings.textInputMethod,
                        });
                    } catch (e) {
                        _error.value = `Injection failed: ${String(e)}`;
                        console.error('Text injection error:', e);
                    }
                }
            }),
            listen<WakeCommandEvent>('wake-command', event => {
                console.log('Wake command:', event.payload.command);
            }),
            listen<AudioErrorEvent>('audio-error', event => {
                _error.value = event.payload.message;
                console.error('Audio error:', event.payload.message);
            }),
            listen<ModelStatus>('models-status-changed', event => {
                _modelStatus.value = event.payload;
            }),
            // Hotkey: hold-to-talk (default mode)
            listen('hotkey-pressed', async () => {
                if (_mode.value === AudioMode.Idle) {
                    try {
                        await invoke('start_listening');
                    } catch (e) {
                        _error.value = String(e);
                    }
                }
            }),
            listen('hotkey-released', async () => {
                if (_mode.value !== AudioMode.Idle) {
                    try {
                        await invoke('stop_listening');
                    } catch (e) {
                        _error.value = String(e);
                    }
                }
            }),
        ]);

        _unlisten.value = listeners;
    }

    function cleanup() {
        for (const fn_ of _unlisten.value) {
            fn_();
        }
        _unlisten.value = [];
    }

    return {
        mode,
        isSpeech,
        lastTranscription,
        error,
        modelStatus,
        isListening,
        isDictation,
        isProcessing,
        isIdle,
        startListening,
        stopListening,
        checkModels,
        testCapture,
        setupEventListeners,
        cleanup,
        __devSetMode,
    };
});
