// provider.ts
import { EventEmitter } from "events";
import { IRequestOptions } from "./types";

interface IProvider {
    send(request: IRequestOptions): Promise<any>;
}

export abstract class Provider extends EventEmitter implements IProvider {
    public abstract send(request: IRequestOptions): Promise<any>;
}
