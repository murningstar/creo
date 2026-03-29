export type SubcommandTierKind = 'dtw' | 'vosk' | 'llm';

export interface SubcommandDef {
    name: string;
    action: string;
    tier: SubcommandTierKind;
    phrases?: string[];
    template?: ParametricTemplate;
}

export interface ParametricTemplate {
    pattern: string;
    slots: SlotDef[];
}

export interface SlotDef {
    name: string;
    description: string;
    examples?: string[];
}

export interface SubcommandManifest {
    commands: SubcommandDef[];
}
