import { WebSocketProvider, MessageListener } from "../WSProvider";

// Mock the WebSocket class
class MockWebSocket {
    url: string;
    listeners: Record<string, ((event: any) => void)[]>;
    lastSentData?: string;

    constructor(url: string) {
        this.url = url;
        this.listeners = {
            message: [],
            error: [],
            close: [],
        };
    }

    addEventListener(type: string, listener: (event: any) => void): void {
        this.listeners[type].push(listener);
    }

    removeEventListener(type: string, listener: (event: any) => void): void {
        this.listeners[type] = this.listeners[type].filter(l => l !== listener);
    }

    send(data: string): void {
        this.lastSentData = data;
    }

    simulateEvent(type: string, event: any): void {
        this.listeners[type].forEach(listener => listener(event));
    }

    close(): void {
        this.simulateEvent("close", {});
    }
}

describe("WebSocketProvider", () => {
    let mockWebSocket: MockWebSocket;

    beforeEach(() => {
        mockWebSocket = new MockWebSocket("ws://test");
        (global as any).WebSocket = jest.fn().mockImplementation(() => mockWebSocket);
    });

    test("constructor", () => {
        const provider = new WebSocketProvider("ws://test");

        expect((global as any).WebSocket).toHaveBeenCalledWith("ws://test");
    });

    test("sendMessage", () => {
        const provider = new WebSocketProvider("ws://test");
        const message = { hello: "world" };

        provider.sendMessage(message);

        expect(mockWebSocket.lastSentData).toEqual(JSON.stringify(message));
    });

    test("addMessageListener", () => {
        const provider = new WebSocketProvider("ws://test");
        const listener: MessageListener = jest.fn();

        provider.addMessageListener(listener);

        const message = { hello: "world" };
        mockWebSocket.simulateEvent("message", { data: JSON.stringify(message) });

        expect(listener).toHaveBeenCalledWith(message);
    });

    test("removeMessageListener", () => {
        const provider = new WebSocketProvider("ws://test");
        const listener: MessageListener = jest.fn();

        provider.addMessageListener(listener);
        provider.removeMessageListener(listener);

        const message = { hello: "world" };
        mockWebSocket.simulateEvent("message", { data: JSON.stringify(message) });

        expect(listener).not.toHaveBeenCalled();
    });

    test("close", () => {
        const provider = new WebSocketProvider("ws://test");

        provider.close();

        expect(mockWebSocket.listeners.close.length).toBe(1);
    });
});