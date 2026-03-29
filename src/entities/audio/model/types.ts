export enum AudioMode {
    Off = 'off',
    Standby = 'standby',
    Dictation = 'dictation',
    Processing = 'processing',
    AwaitingSubcommand = 'awaiting_subcommand',
}

export type WakeAction = 'await_subcommand' | 'start_dictation' | 'stop_dictation' | 'cancel_dictation';

export interface AudioStateEvent {
    mode: AudioMode;
}

export interface WakeCommandEvent {
    command: WakeAction;
}

export interface TranscriptionEvent {
    text: string;
    isFinal: boolean;
}

export interface VadStateEvent {
    isSpeech: boolean;
}

export interface AudioErrorEvent {
    message: string;
}

export interface SttEngineResolvedEvent {
    engine: string;
}

export interface SubcommandMatchEvent {
    command: string;
    action: string;
    confidence: number;
    tier: number;
    params: Record<string, string>;
}

export interface ModelInfo {
    name: string;
    filename: string;
    path: string;
    exists: boolean;
    sizeHint: string;
}

export interface ModelStatus {
    modelsDir: string;
    allPresent: boolean;
    models: ModelInfo[];
}
