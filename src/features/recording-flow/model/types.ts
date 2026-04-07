import type { WakeAction } from '~/entities/wake-commands';

export interface RecordingCommand {
    name: string;
    label: string;
    action?: WakeAction;
    requiredSamples: number;
}
