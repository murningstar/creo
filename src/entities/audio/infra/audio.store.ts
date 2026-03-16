import { AudioMode } from '../model/types';

export const useAudioStore = defineStore('audio', () => {
    const _mode = ref<AudioMode>(AudioMode.Idle);

    const mode = readonly(_mode);

    const isListening = computed<boolean>(() => _mode.value === AudioMode.Listening);

    const isDictation = computed<boolean>(() => _mode.value === AudioMode.Dictation);

    const isProcessing = computed<boolean>(() => _mode.value === AudioMode.Processing);

    const isIdle = computed<boolean>(() => _mode.value === AudioMode.Idle);

    const _setMode = (newMode: AudioMode) => {
        _mode.value = newMode;
    };

    return {
        mode,
        isListening,
        isDictation,
        isProcessing,
        isIdle,
        _setMode,
    };
});
