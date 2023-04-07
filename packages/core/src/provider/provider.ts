import { EventEmitter } from "events";
import { Query } from "../types";

interface IProvider {
    set_entity(component: number,
        query: Query,
        offset: number,
        value: number[],
        calldata?: any[]): Promise<any>;
    get_component(component: string, entity_id: string, offset: string, length: string): Promise<number>;
    get_entity(entity_id: string): Promise<number>;
    get_entities(entites: any[]): Promise<number[]>;
}

export abstract class Provider extends EventEmitter implements IProvider {
    private readonly worldAddress: string;

    constructor(worldAddress: string) {
        super();
        this.worldAddress = worldAddress;
    }

    // components
    public abstract get_component(component: string, entity_id: string, offset: string, length: string): Promise<number>;

    // entities
    public abstract set_entity(component: number,
        query: Query,
        offset: number,
        value: number[],
        calldata?: any[]): Promise<any>;
    public abstract get_entity(entity_id: string): Promise<any>;
    public abstract get_entities(entites: any[]): Promise<any>;


    public getWorldAddress(): string {
        return this.worldAddress;
    }
}
