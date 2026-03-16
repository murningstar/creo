type ObjectEntries<O> = {
    [K in keyof O]: [K, O[K]];
}[keyof O][];

export const keysOf = <T extends object>(obj: T) => Object.keys(obj) as (keyof T)[];

export const valuesOf = <T extends object>(obj: T) => Object.values(obj) as T[keyof T][];

export const entriesOf = <T extends object>(obj: T) => Object.entries(obj) as ObjectEntries<T>;

export const objFromEntries = <K extends PropertyKey, V>(arr: readonly (readonly [K, V])[]): { [P in K]: V } =>
    Object.fromEntries(arr) as { [P in K]: V };
