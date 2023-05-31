import { EventEmitter } from "events";
import { ICommands, Query } from "../types";

export abstract class Provider extends EventEmitter implements ICommands {
    private readonly worldAddress: string;

    constructor(worldAddress: string) {
        super();
        this.worldAddress = worldAddress;
    }

    public abstract entity(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>>;

    public abstract entities(component: string, partition: string): Promise<Array<bigint>>;

    public getWorldAddress(): string {
        return this.worldAddress;
    }
}