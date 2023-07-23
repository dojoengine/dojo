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
    entities?(component: string, partition: string, length: number): Promise<Array<bigint>>;
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


// examples types - TODO: These should be codegen'd from the manifest
export type ComponentNames = "" | "Moves" | "Position" | "AuthStatus" | "AuthRole";
export type SystemNames = "" | "spawn" | "move";


export type ExecuteState = 'idle' | 'loading' | 'done' | 'error'

export interface Members {
    name: string;
    type: string;
    slot: number;
    offset: number;
}

export interface RegisteredComponent {
    name: ComponentNames;
    members: Members[];
    class_hash: string;
}

export interface InputOutput {
    name?: string;
    type: string;
}

export interface RegisteredSystem {
    name: SystemNames;
    inputs: InputOutput[];
    outputs: InputOutput[];
    class_hash: string;
    dependencies: string[];
}

export interface Contract { }

export interface Manifest {
    world: string;
    executor: string;
    components: RegisteredComponent[];
    systems: RegisteredSystem[];
    contracts: Contract[];
}

export interface World {
    world: string;
    executor: string;
    components: RegisteredComponent[];
    systems: RegisteredSystem[];
    entities: Record<number, Entity>;
}


export interface CallData {
    componentName: string;
    call_data: Array<bigint>;
}

export interface Component {
    name: string;
    data: any;
}

export interface Entity {
    id: number;
    components: Record<string, Component>;
}


export interface ComponentQuery {
    name: string;
    dataValues?: { [key: string]: any };
}