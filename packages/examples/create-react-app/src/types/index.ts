export enum Entity {
    Realm = 'Realm',
    Army = 'Army'
}

export interface Realm {
    id: number;
    name: string;
    description: string;
    owner: number;
    armies: number[];
}

export interface EntityData {
    entityId: number;
    entityType: Entity;
}

export interface QueryResult<T> {
    data: T | undefined;
    loading: boolean;
    error: any;
}