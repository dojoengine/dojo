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

export interface ICommands {
    // get singular component
    entity?(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>>;
    // composeEntity?(parameters: Array<{ component: string; query: Query; offset: number; length: number }>): Promise<{ [key: string]: Array<bigint> }>

    // get many
    entities?(component: bigint, partition: bigint): Promise<Array<bigint>>;
    // composeEntities?(parameters: Array<{ component: string; query: Query; offset: number; length: number }>): Promise<{ [key: string]: Array<bigint> }>

    // execute
    execute?(name: bigint, execute_calldata: Array<bigint>): Promise<Array<bigint>>;

    // add generic world commands
    blocktime?(): Promise<bigint>;
    worldAge?(): Promise<bigint>;
}