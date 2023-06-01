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

// examples types - TODO: Export in bindings
export type ComponentNames = "" | "Moves" | "Position" | "AuthStatus" | "AuthRole";
export type SystemNames = "" | "SpawnSystem" | "MoveSystem" | "RouteAuthSystem" | "IsAccountAdminSystem" | "IsAuthorizedSystem" | "GrantAuthRoleSystem" | "GrantScopedAuthRoleSystem" | "GrantResourceSystem" | "RevokeAuthRoleSystem" | "RevokeScopedAuthRoleSystem" | "RevokeResourceSystem";


export interface Members {
    name: string;
    type: string;
    slot: number;
    offset: number;
}

export interface Component {
    name: ComponentNames;
    members: Members[];
    class_hash: string;
}

export interface InputOutput {
    name?: string;
    type: string;
}

export interface System {
    name: SystemNames;
    inputs: InputOutput[];
    outputs: InputOutput[];
    class_hash: string;
    dependencies: string[];
}

export interface Contract { } // Add fields as per your Contract object's structure

export interface Manifest {
    world: string;
    executor: string;
    components: Component[];
    systems: System[];
    contracts: Contract[];
}

export interface World {
    world: string;
    executor: string;
}


export interface CallData {
    componentName: string;
    call_data: Array<bigint>;
}