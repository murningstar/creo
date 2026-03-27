import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { AudioMode, type AudioStateEvent, type TranscriptionEvent, type WakeCommandEvent } from '~/entities/audio';
import { useSettingsStore } from '~/entities/settings';

import { DictationPhase } from '../model/types';

const FINISHING_TIMEOUT_MS = 500;

// Module-level singleton state
const _phase = ref<DictationPhase>(DictationPhase.Inactive);
const _lastInjectedText = ref('');
const _error = ref<string | null>(null);
const _unlisten = ref<UnlistenFn[]>([]);
const _finishingTimeout = ref<ReturnType<typeof setTimeout> | null>(null);

function clearFinishingTimeout() {
    if (_finishingTimeout.value) {
        clearTimeout(_finishingTimeout.value);
        _finishingTimeout.value = null;
    }
}

function transitionToInactive() {
    clearFinishingTimeout();
    _phase.value = DictationPhase.Inactive;
}

function startFinishingTimeout() {
    clearFinishingTimeout();
    _finishingTimeout.value = setTimeout(() => {
        if (_phase.value === DictationPhase.Finishing) {
            transitionToInactive();
        }
    }, FINISHING_TIMEOUT_MS);
}

export function useDictationFlow() {
    const settingsStore = useSettingsStore();

    async function setupListeners() {
        // Guard: cleanup existing listeners before re-registering (handles HMR / reload)
        if (_unlisten.value.length > 0) {
            cleanup();
        }

        const listeners = await Promise.all([
            // Hotkey: mode-aware (hold or toggle)
            listen('hotkey-pressed', async () => {
                const mode = settingsStore.hotkeyMode;

                if (mode === 'toggle') {
                    // Toggle: press once to start, press again to stop
                    if (_phase.value === DictationPhase.Active) {
                        _phase.value = DictationPhase.Finishing;
                        startFinishingTimeout();
                        try {
                            await invoke('transition_to_standby');
                        } catch (e) {
                            _error.value = String(e);
                            transitionToInactive();
                        }
                        return;
                    }
                }

                // Hold: start on press. Toggle: start on press (when inactive).
                if (_phase.value !== DictationPhase.Inactive) return;

                _phase.value = DictationPhase.Starting;
                _error.value = null;
                try {
                    await invoke('transition_to_dictation');
                } catch (e) {
                    _error.value = String(e);
                    _phase.value = DictationPhase.Inactive;
                }
            }),

            listen('hotkey-released', async () => {
                // Toggle mode ignores release
                if (settingsStore.hotkeyMode === 'toggle') return;

                if (_phase.value !== DictationPhase.Active && _phase.value !== DictationPhase.Starting) return;

                _phase.value = DictationPhase.Finishing;
                startFinishingTimeout();
                try {
                    await invoke('transition_to_standby');
                } catch (e) {
                    _error.value = String(e);
                    transitionToInactive();
                }
            }),

            // Pipeline confirmed Dictation mode — transition Starting → Active
            listen<AudioStateEvent>('audio-state-changed', event => {
                if (event.payload.mode === AudioMode.Dictation && _phase.value === DictationPhase.Starting) {
                    _phase.value = DictationPhase.Active;
                }
            }),

            // Transcription arrived — inject text
            listen<TranscriptionEvent>('transcription', async event => {
                if (!event.payload.isFinal) return;
                if (_phase.value !== DictationPhase.Active && _phase.value !== DictationPhase.Finishing) return;

                try {
                    await invoke('inject_text', {
                        text: event.payload.text,
                        method: settingsStore.textInputMethod,
                    });
                    _lastInjectedText.value = event.payload.text;
                } catch (e) {
                    _error.value = `Injection failed: ${String(e)}`;
                    console.error('Text injection error:', e);
                }

                // If we were finishing (hotkey released), we're done
                if (_phase.value === DictationPhase.Finishing) {
                    transitionToInactive();
                }
            }),

            // Wake command: voice-activated dictation start/stop
            listen<WakeCommandEvent>('wake-command', event => {
                if (event.payload.command === 'start_dictation' && _phase.value === DictationPhase.Inactive) {
                    // Pipeline already transitioned to Dictation mode
                    _phase.value = DictationPhase.Active;
                }

                if (
                    (event.payload.command === 'stop_dictation' || event.payload.command === 'cancel_dictation') &&
                    _phase.value === DictationPhase.Active
                ) {
                    // Pipeline handles transition to Standby
                    transitionToInactive();
                }
            }),
        ]);

        _unlisten.value = listeners;
    }

    function cleanup() {
        clearFinishingTimeout();
        for (const fn_ of _unlisten.value) {
            fn_();
        }
        _unlisten.value = [];
    }

    return {
        phase: readonly(_phase),
        lastInjectedText: readonly(_lastInjectedText),
        error: readonly(_error),
        setupListeners,
        cleanup,
    };
}
