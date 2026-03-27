export const REQUIRED_SAMPLES = 3;

// Mirrors WakeAction in entities/audio/model/types.ts — kept separate per FSD (no cross-entity imports).
// If adding a new action, update BOTH types.
export type WakeActionType = 'await_subcommand' | 'start_dictation' | 'stop_dictation' | 'cancel_dictation';

export interface WakeActionOption {
    value: WakeActionType;
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

export interface RecordResult {
    commandName: string;
    embeddingCount: number;
    totalSamples: number;
    path: string;
}

// Base commands are tied to the assistant name and must be re-recorded on rename.
// Phrase template: "{assistantName}, {suffix}"

export interface BaseCommandDef {
    action: WakeActionType;
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

export function buildBaseCommandName(assistantName: string, suffix: string): string {
    return `${assistantName}, ${suffix}`;
}

export function getBaseCommandNames(assistantName: string): string[] {
    return BASE_COMMANDS.map(cmd => buildBaseCommandName(assistantName, cmd.suffix));
}
