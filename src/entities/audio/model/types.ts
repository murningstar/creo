export enum AudioMode {
    Idle = 'idle',
    Listening = 'listening',
    Dictation = 'dictation',
    Processing = 'processing',
}

export type WakeAction = 'command_mode' | 'start_dictation' | 'stop_dictation';

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
