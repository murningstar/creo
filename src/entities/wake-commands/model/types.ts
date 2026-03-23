export type WakeActionType = 'command_mode' | 'start_dictation' | 'stop_dictation';

export interface WakeActionOption {
    value: WakeActionType;
    label: string;
    description: string;
}

export const WAKE_ACTION_OPTIONS: WakeActionOption[] = [
    { value: 'command_mode', label: 'Command Mode', description: 'Activate command menu' },
    { value: 'start_dictation', label: 'Start Dictation', description: 'Begin voice-to-text input' },
    { value: 'stop_dictation', label: 'Stop Dictation', description: 'Finish voice-to-text input' },
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
