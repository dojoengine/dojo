/// <reference types="node" />
import { EventEmitter } from "events";
import { IRequestOptions } from "./types";
interface IProvider {
    send(request: IRequestOptions): Promise<any>;
}
export declare abstract class Provider extends EventEmitter implements IProvider {
    abstract send(request: IRequestOptions): Promise<any>;
}
export {};
