export enum WorldEntryPoints {
    get = "entity",
    set = "set_entity",
    entities = "entities",
    execute = "execute",
    register_system = "register_system",
    register_component = "register_component",
    component = "component",
    system = "system"
}

export interface Query {
    address_domain: string,
    partition: string,
    keys: bigint[]
}

export interface ICommands {

    entity?(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>>;
    entities?(component: string, partition: string): Promise<Array<bigint>>;
    execute?(name: bigint, execute_calldata: Array<bigint>): Promise<Array<bigint>>;

    register_component?(class_hash: string): Promise<bigint>;
    register_system?(class_hash: string): Promise<bigint>;

    // views
    is_authorized?(system: string, component: string): Promise<bigint>;
    is_account_admin?(): Promise<bigint>;

    component?(name: string): Promise<bigint>;
    system?(name: string): Promise<bigint>;

    // add generic world commands
    blocktime?(): Promise<bigint>;
    worldAge?(): Promise<bigint>;
}