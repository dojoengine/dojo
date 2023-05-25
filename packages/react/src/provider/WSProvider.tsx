import React, { createContext, useContext, useState, useEffect } from "react";
import { Providers } from '@dojoengine/core';
import { MessageListener } from "@dojoengine/core/dist/provider/WSProvider";

type WebSocketContextType = {
    sendMessage: (message: any) => void;
    addMessageListener: (listener: MessageListener) => void;
    removeMessageListener: (listener: MessageListener) => void;
};

const WebSocketContext = createContext<WebSocketContextType | null>(null);

interface WebSocketProviderProps {
    ws: string;
    children: React.ReactNode;
}

function WebSocketProvider({ ws, children }: WebSocketProviderProps) {
    const [wsProvider, setWsProvider] = useState<Providers.WebSocketProvider | null>(null);

    useEffect(() => {
        const provider = new Providers.WebSocketProvider(ws);
        setWsProvider(provider);

        provider.ws.addEventListener('open', () => {
            provider.sendMessage("Hello, Dojo");
        });

        return () => {
            if (wsProvider) {
                wsProvider.close();
            }
        };
    }, [ws]);

    if (!wsProvider) {
        return null;
    }

    const value: WebSocketContextType = {
        sendMessage: wsProvider.sendMessage.bind(wsProvider),
        addMessageListener: wsProvider.addMessageListener.bind(wsProvider),
        removeMessageListener: wsProvider.removeMessageListener.bind(wsProvider),
    };

    return (
        <WebSocketContext.Provider value={value}>
            {children}
        </WebSocketContext.Provider>
    );
};

const useWebSocketContext = (): WebSocketContextType => {
    const context = useContext(WebSocketContext);
    if (!context) {
        throw new Error("useWebSocketContext must be used within a WebSocketProvider");
    }
    return context;
};

export { WebSocketProvider, useWebSocketContext };