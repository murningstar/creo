import type { WakeActionType } from '~/entities/wake-commands';

export interface RecordingCommand {
    name: string;
    label: string;
    action: WakeActionType;
    requiredSamples: number;
}
