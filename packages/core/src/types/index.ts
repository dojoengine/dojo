import { num } from "starknet";

/**
 * Enumeration representing various entry points or functions available in the World.
 */
export enum WorldEntryPoints {
    get = "entity",  // Retrieve a single entity
    set = "set_entity",  // Set or update a single entity
    entities = "entities",  // Retrieve multiple entities
    execute = "execute",  // Execute a specific command
    register_system = "register_system",  // Register a new system
    register_component = "register_component",  // Register a new component
    component = "component",  // Access a component
    system = "system"  // Access a system
}

/**
 * Interface representing a query structure with domain and keys.
 */
export interface Query {
    keys: num.BigNumberish[]  // A list of keys used in the query
}

/**
 * ICommands interface provides a set of optional command methods that can be implemented 
 * by classes to interact with the World system.
 */
export interface ICommands {
    // Retrieve details of a single entity
    entity?(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>>;

    // Retrieve details of multiple entities
    entities?(component: string, length: number): Promise<Array<bigint>>;

    // Execute a specific command
    execute?(name: bigint, execute_calldata: Array<bigint>): Promise<Array<bigint>>;

    // Register a new component and return its ID
    register_component?(class_hash: string): Promise<bigint>;

    // Register a new system and return its ID
    register_system?(class_hash: string): Promise<bigint>;

    // Check if a system is authorized to access a component
    is_authorized?(system: string, component: string): Promise<bigint>;

    // Check if the current user/account is an admin
    is_account_admin?(): Promise<bigint>;

    // Access a specific component and return its details
    component?(name: string): Promise<bigint>;

    // Access a specific system and return its details
    system?(name: string): Promise<bigint>;

    // Get the current block time
    blocktime?(): Promise<bigint>;

    // Get the age or duration since the World was created
    worldAge?(): Promise<bigint>;
}