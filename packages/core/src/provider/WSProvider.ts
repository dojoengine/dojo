export type MessageListener = (message: any) => void;

export class WebSocketProvider {
    public ws: WebSocket;
    public listeners: MessageListener[];

    constructor(ws: string) {
        this.ws = new WebSocket(ws);
        this.listeners = [];

        this.ws.addEventListener("message", (event) => {
            const message = JSON.parse(event.data);
            this.listeners.forEach((listener) => listener(message));
        });

        this.ws.addEventListener("error", (event) => {
            console.error("WebSocket error:", event);
        });

        this.ws.addEventListener("close", (event) => {
            console.log("WebSocket closed:", event);
        });
    }

    sendMessage(message: any): void {
        if (this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
        } else {
            console.error("WebSocket is not open:", this.ws.readyState);
        }
    }

    addMessageListener(listener: MessageListener): void {
        this.listeners.push(listener);
    }

    removeMessageListener(listener: MessageListener): void {
        this.listeners = this.listeners.filter((l) => l !== listener);
    }

    close(): void {
        this.ws.close();
    }
}