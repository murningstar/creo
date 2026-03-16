export enum AudioMode {
    Idle = 'idle',
    Listening = 'listening',
    Dictation = 'dictation',
    Processing = 'processing',
}

export interface WakeCommand {
    keyword: string;
    action: string;
}
