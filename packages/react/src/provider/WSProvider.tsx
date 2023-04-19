// WebSocketContext.tsx
import React, { createContext, useContext, useEffect, useState } from "react";
import { Providers } from '@dojoengine/core';

type WebSocketContextType = {
    sendMessage: (message: any) => void;
    addMessageListener: (listener: (message: any) => void) => void;
    removeMessageListener: (listener: (message: any) => void) => void;
};

const WebSocketContext = createContext<WebSocketContextType | null>(null);

interface WebSocketProviderProps {
    url: string;
    children: React.ReactNode;
}

const WebSocketProviderComponent: React.FC<WebSocketProviderProps> = ({ url, children }) => {
    const [wsProvider, setWsProvider] = useState<Providers.WebSocketProvider | null>(null);

    useEffect(() => {
        const provider = new Providers.WebSocketProvider(url);
        setWsProvider(provider);

        return () => {
            provider.close();
        };
    }, [url]);

    if (!wsProvider) {
        return null;
    }

    const value: WebSocketContextType = {
        sendMessage: wsProvider.sendMessage,
        addMessageListener: wsProvider.addMessageListener,
        removeMessageListener: wsProvider.removeMessageListener,
    };

    return <WebSocketContext.Provider value={value}>{children}</WebSocketContext.Provider>;
};

const useWebSocketContext = (): WebSocketContextType => {
    const context = useContext(WebSocketContext);
    if (!context) {
        throw new Error("useWebSocketContext must be used within a WebSocketProvider");
    }
    return context;
};

export { WebSocketProviderComponent, useWebSocketContext };