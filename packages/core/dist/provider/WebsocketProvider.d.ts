import { Provider } from "./provider";
import { IRequestOptions } from "./types";
export declare class WebsocketProvider extends Provider {
    private websocket;
    constructor(url: string);
    send(request: IRequestOptions): Promise<any>;
    private onMessage;
    private onError;
}
