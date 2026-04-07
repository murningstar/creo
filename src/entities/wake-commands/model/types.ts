import type { WakeAction } from '~/shared/model/types';

export type { WakeAction } from '~/shared/model/types';
export type { RecordResult } from '~/shared/model/types';

export const REQUIRED_SAMPLES = 3;

export interface WakeActionOption {
    value: WakeAction;
    label: string;
    description: string;
}

export const WAKE_ACTION_OPTIONS: WakeActionOption[] = [
    { value: 'await_subcommand', label: 'Await Subcommand', description: 'Wait for a follow-up command' },
    { value: 'start_dictation', label: 'Start Dictation', description: 'Begin voice-to-text input' },
    { value: 'stop_dictation', label: 'Stop Dictation', description: 'Finish voice-to-text input' },
    { value: 'cancel_dictation', label: 'Cancel Dictation', description: 'Abort dictation without injecting text' },
];

export interface WakeCommandInfo {
    name: string;
    sampleCount: number;
}

// Base commands are tied to the assistant name and must be re-recorded on rename.
// Phrase template: "{assistantName}, {suffix}"

export interface BaseCommandDef {
    action: WakeAction;
    suffix: string;
    label: string;
    instruction: string;
}

export const BASE_COMMANDS: BaseCommandDef[] = [
    {
        action: 'await_subcommand',
        suffix: 'приём',
        label: 'Wake command',
        instruction: 'Activates the assistant',
    },
    {
        action: 'start_dictation',
        suffix: 'вписывай',
        label: 'Start dictation',
        instruction: 'Begins voice-to-text input',
    },
    {
        action: 'stop_dictation',
        suffix: 'готово',
        label: 'Stop dictation',
        instruction: 'Finishes voice-to-text input',
    },
    {
        action: 'cancel_dictation',
        suffix: 'отмена',
        label: 'Cancel dictation',
        instruction: 'Aborts dictation without injecting text',
    },
];
