import { ICommands, Query } from "../types";

export abstract class Provider implements ICommands {
    private readonly worldAddress: string;

    constructor(worldAddress: string) {
        this.worldAddress = worldAddress;
    }

    public abstract entity(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>>;

    public abstract entities(component: string, length: number): Promise<Array<bigint>>;

    public getWorldAddress(): string {
        return this.worldAddress;
    }
}