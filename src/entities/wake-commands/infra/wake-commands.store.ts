import { invoke } from '@tauri-apps/api/core';

import type { RecordResult, WakeActionType, WakeCommandInfo } from '../model/types';

export const useWakeCommandsStore = defineStore('wake-commands', () => {
    const _commands = ref<WakeCommandInfo[]>([]);
    const _isRecording = ref(false);
    const _error = ref<string | null>(null);

    const commands = readonly(_commands);
    const isRecording = readonly(_isRecording);
    const error = readonly(_error);

    const hasCommands = computed<boolean>(() => _commands.value.length > 0);

    async function loadCommands() {
        try {
            _commands.value = await invoke<WakeCommandInfo[]>('get_wake_commands');
        } catch (e) {
            _error.value = String(e);
        }
    }

    async function recordSample(commandName: string, action?: WakeActionType): Promise<RecordResult | null> {
        _error.value = null;
        _isRecording.value = true;
        try {
            const result = await invoke<RecordResult>('record_wake_sample', { commandName, action });
            // Refresh command list after recording
            await loadCommands();
            return result;
        } catch (e) {
            _error.value = String(e);
            return null;
        } finally {
            _isRecording.value = false;
        }
    }

    async function deleteCommand(commandName: string) {
        _error.value = null;
        try {
            await invoke('delete_wake_command', { commandName });
            await loadCommands();
        } catch (e) {
            _error.value = String(e);
        }
    }

    return {
        commands,
        isRecording,
        error,
        hasCommands,
        loadCommands,
        recordSample,
        deleteCommand,
    };
});
