import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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
    const _mode = ref<AudioMode>(AudioMode.Off);
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

    const isStandby = computed<boolean>(() => _mode.value === AudioMode.Standby);
    const isDictation = computed<boolean>(() => _mode.value === AudioMode.Dictation);
    const isProcessing = computed<boolean>(() => _mode.value === AudioMode.Processing);
    const isOff = computed<boolean>(() => _mode.value === AudioMode.Off);
    const isAwaitingSubcommand = computed<boolean>(() => _mode.value === AudioMode.AwaitingSubcommand);

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

    /** Sync frontend mode with current Rust pipeline state (for reload recovery). */
    async function syncMode() {
        try {
            const modeStr = await invoke<string>('get_current_mode');
            const mode = JSON.parse(modeStr) as AudioMode;
            _mode.value = mode;
        } catch {
            // Not running in Tauri or pipeline not initialized — stay Off
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
        // Guard: cleanup existing listeners before re-registering (handles HMR / reload)
        if (_unlisten.value.length > 0) {
            cleanup();
        }

        const listeners = await Promise.all([
            listen<AudioStateEvent>('audio-state-changed', event => {
                _mode.value = event.payload.mode;
            }),
            listen<VadStateEvent>('vad-state', event => {
                _isSpeech.value = event.payload.isSpeech;
            }),
            listen<TranscriptionEvent>('transcription', event => {
                _lastTranscription.value = event.payload.text;
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
        isStandby,
        isDictation,
        isProcessing,
        isOff,
        isAwaitingSubcommand,
        startListening,
        stopListening,
        checkModels,
        syncMode,
        testCapture,
        setupEventListeners,
        cleanup,
        __devSetMode,
    };
});
