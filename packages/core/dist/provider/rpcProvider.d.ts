import { Provider } from "./provider";
import { StorageKey } from "../types";
export declare class RPCProvider extends Provider {
    private provider;
    constructor(world_address: string, url: string);
    set_entity(component: number, key: StorageKey, offset: number, value: number[], calldata?: any[]): Promise<any>;
    get_component(component: string, entity_id: string, offset: string, length: string): Promise<any>;
    get_entity(): Promise<any[]>;
    get_entities(): Promise<any[]>;
}
