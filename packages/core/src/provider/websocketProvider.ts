import { Provider } from "./provider";
import { IRequestOptions } from "./types";

// websocketProvider.ts
export class WebsocketProvider extends Provider {
    private websocket: WebSocket;

    constructor(url: string) {
        super();
        this.websocket = new WebSocket(url);
        this.websocket.onmessage = (event: MessageEvent) => this.onMessage(event);
        this.websocket.onerror = (event: Event) => this.onError(event);
    }

    public send(request: IRequestOptions): Promise<any> {
        return new Promise((resolve, reject) => {
            this.websocket.send(JSON.stringify(request));
            this.once("response", resolve);
            this.once("error", reject);
        });
    }

    private onMessage(event: MessageEvent): void {
        const data = JSON.parse(event.data);
        this.emit("response", data);
    }

    private onError(event: Event): void {
        this.emit("error", new Error("WebSocket error:"));
    }
}