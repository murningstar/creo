/** Wire-format type for wake command actions. Matches Rust WakeAction serde values. */
export type WakeAction = 'await_subcommand' | 'start_dictation' | 'stop_dictation' | 'cancel_dictation';

/** Result of recording a voice sample via Tauri IPC. */
export interface RecordResult {
    commandName: string;
    embeddingCount: number;
    totalSamples: number;
    path: string;
}
