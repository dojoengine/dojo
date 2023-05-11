export enum WorldEntryPoints {
    get = "entity",
    set = "set_entity",
    entities = "entities",
    execute = "execute"
}

export interface Query {
    partition: string,
    keys: string[]
}

// TODO: add individual interfaces for each of the entrypoints
export interface IWorld {
    register_component?(string: string): void;
    component?(name: bigint): Promise<string>;
    register_system?(string: string): void;
    system?(name: bigint): Promise<string>;
    execute?(name: bigint, execute_calldata: Array<bigint>): Promise<Array<bigint>>;
    uuid?(): Promise<bigint>;
    set_entity?(component: bigint, query: Query, offset: number, value: Array<bigint>): void;
    delete_entity?(component: bigint, query: Query): void;
    entity?(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>>;
    entities?(component: bigint, partition: bigint): Promise<Array<bigint>>;
    set_executor?(string: string): void;
    has_role?(role: bigint, account: string): Promise<boolean>;
    grant_role?(role: bigint, account: string): void;
    revoke_role?(role: bigint, account: string): void;
    renounce_role?(role: bigint): void;
}