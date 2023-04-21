// WebSocketContext.tsx
import React, { createContext, useContext, Component } from "react";
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

class WebSocketProvider extends Component<WebSocketProviderProps, { wsProvider: Providers.WebSocketProvider | null }> {
    constructor(props: WebSocketProviderProps) {
        super(props);
        this.state = {
            wsProvider: null,
        };
    }

    componentDidMount() {
        const provider = new Providers.WebSocketProvider(this.props.ws);
        this.setState({ wsProvider: provider });

        provider.ws.addEventListener('open', () => {
            provider.sendMessage("Hello, Dojo");
        });
    }

    componentWillUnmount() {
        if (this.state.wsProvider) {
            this.state.wsProvider.close();
        }
    }

    render() {
        if (!this.state.wsProvider) {
            return null;
        }

        const value: WebSocketContextType = {
            sendMessage: this.state.wsProvider.sendMessage.bind(this.state.wsProvider),
            addMessageListener: this.state.wsProvider.addMessageListener.bind(this.state.wsProvider),
            removeMessageListener: this.state.wsProvider.removeMessageListener.bind(this.state.wsProvider),
        };

        return (
            <WebSocketContext.Provider value={value}>
                {this.props.children}
            </WebSocketContext.Provider>
        );
    }
}

const useWebSocketContext = (): WebSocketContextType => {
    const context = useContext(WebSocketContext);
    if (!context) {
        throw new Error("useWebSocketContext must be used within a WebSocketProvider");
    }
    return context;
};

export { WebSocketProvider, useWebSocketContext };