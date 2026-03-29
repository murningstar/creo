import { invoke } from '@tauri-apps/api/core';

import type { RecordResult } from '~/entities/wake-commands';

import type { SubcommandDef, SubcommandManifest } from '../model/types';

export const useSubcommandsStore = defineStore('subcommands', () => {
    const _commands = ref<SubcommandDef[]>([]);
    const _isRecording = ref(false);
    const _error = ref<string | null>(null);

    const commands = readonly(_commands);
    const isRecording = readonly(_isRecording);
    const error = readonly(_error);
    const hasCommands = computed<boolean>(() => _commands.value.length > 0);

    async function loadCommands() {
        try {
            const manifest = await invoke<SubcommandManifest>('get_subcommands');
            _commands.value = manifest.commands;
        } catch (e) {
            _error.value = String(e);
        }
    }

    async function createCommand(name: string, action: string, tier: string) {
        _error.value = null;
        try {
            await invoke('create_subcommand', { name, action, tier });
            await loadCommands();
        } catch (e) {
            _error.value = String(e);
        }
    }

    async function deleteCommand(name: string) {
        _error.value = null;
        try {
            await invoke('delete_subcommand', { name });
            await loadCommands();
        } catch (e) {
            _error.value = String(e);
        }
    }

    async function recordSample(commandName: string): Promise<RecordResult | null> {
        _error.value = null;
        _isRecording.value = true;
        try {
            const result = await invoke<RecordResult>('record_subcommand_sample', { commandName });
            await loadCommands();
            return result;
        } catch (e) {
            _error.value = String(e);
            return null;
        } finally {
            _isRecording.value = false;
        }
    }

    return {
        commands,
        isRecording,
        error,
        hasCommands,
        loadCommands,
        createCommand,
        deleteCommand,
        recordSample,
    };
});
