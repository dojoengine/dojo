/// <reference types="node" />
import { EventEmitter } from "events";
import { StorageKey } from "../types";
interface IProvider {
    set_entity(component: number, key: StorageKey, offset: number, value: number[], calldata?: any[]): Promise<any>;
    get_component(component: string, entity_id: string, offset: string, length: string): Promise<number>;
    get_entity(entity_id: string): Promise<number>;
    get_entities(entites: any[]): Promise<number[]>;
}
export declare abstract class Provider extends EventEmitter implements IProvider {
    private readonly worldAddress;
    constructor(worldAddress: string);
    abstract get_component(component: string, entity_id: string, offset: string, length: string): Promise<number>;
    abstract set_entity(component: number, key: StorageKey, offset: number, value: number[], calldata?: any[]): Promise<any>;
    abstract get_entity(entity_id: string): Promise<any>;
    abstract get_entities(entites: any[]): Promise<any>;
    getWorldAddress(): string;
}
export {};
