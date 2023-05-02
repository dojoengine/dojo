import { EventEmitter } from "events";
import { IWorld, Query } from "../types";

export abstract class Provider extends EventEmitter implements IWorld {
    private readonly worldAddress: string;

    constructor(worldAddress: string) {
        super();
        this.worldAddress = worldAddress;
    }

    public abstract entity(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>>;

    public getWorldAddress(): string {
        return this.worldAddress;
    }

    // TODO: Global systems, any function needed to interact with a Dojo world should exist here

    // TODO: get all worlds components

    // TODO: get all worlds systems

}