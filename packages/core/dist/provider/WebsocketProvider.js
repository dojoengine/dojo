"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.WebsocketProvider = void 0;
const provider_1 = require("./provider");
// websocketProvider.ts
class WebsocketProvider extends provider_1.Provider {
    constructor(url) {
        super();
        this.websocket = new WebSocket(url);
        this.websocket.onmessage = (event) => this.onMessage(event);
        this.websocket.onerror = (event) => this.onError(event);
    }
    send(request) {
        return new Promise((resolve, reject) => {
            this.websocket.send(JSON.stringify(request));
            this.once("response", resolve);
            this.once("error", reject);
        });
    }
    onMessage(event) {
        const data = JSON.parse(event.data);
        this.emit("response", data);
    }
    onError(event) {
        this.emit("error", new Error("WebSocket error:"));
    }
}
exports.WebsocketProvider = WebsocketProvider;
