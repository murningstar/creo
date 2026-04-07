import { BASE_COMMANDS } from '../model/types';

export function buildBaseCommandName(assistantName: string, suffix: string): string {
    return `${assistantName}, ${suffix}`;
}

export function getBaseCommandNames(assistantName: string): string[] {
    return BASE_COMMANDS.map(cmd => buildBaseCommandName(assistantName, cmd.suffix));
}
