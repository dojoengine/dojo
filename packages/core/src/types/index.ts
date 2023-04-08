export enum WorldEntryPoints {
    get = "get",
    set = "set",
    entities = "entities",
    execute = "execute"
}

export interface Query {
    partition: string,
    keys: string[]
}

// TODO: extend Provider to this
export interface IWorld {
    constructor(executor_: string, store_: string, indexer_: string): void;
    register_component(class_hash: string): void;
    component(name: number): string;
    register_system(class_hash: string): void;
    system(name: number): string;
    execute(name: number, execute_calldata: number[]): number[];
    uuid(): number;
    set(component: number, query: Query, offset: number, value: number[]): void;
    get(component: number, query: Query, offset: number, length: number): number[];
    entities(component: number, partition: number): number[];
    set_executor(contract_address: string): void;
    set_indexer(class_hash: string): void;
    set_store(class_hash: string): void;
    has_role(role: number, account: string): boolean;
    grant_role(role: number, account: string): void;
    revoke_role(role: number, account: string): void;
    renounce_role(role: number): void;
  }