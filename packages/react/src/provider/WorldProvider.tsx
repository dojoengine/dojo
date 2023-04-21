import React, { createContext, useContext, ReactNode } from 'react';
import ControllerConnector from "@cartridge/connector";
import {
    InjectedConnector,
    StarknetConfig,
    useConnectors,
} from "@starknet-react/core";
import { RPCProvider } from './RPCProvider';
import { WebSocketProvider } from './WSProvider';

export interface WorldContextValue {
    connectors: any[];
}

const WorldContext = createContext<WorldContextValue | null>(null);

interface WorldProviderProps {
    worldAddress: string;
    rpcUrl?: string;
    ws: string;
    children: ReactNode;
    connectors: any[];
}

export function WorldProvider({ worldAddress, rpcUrl, children, connectors, ws }: WorldProviderProps) {

    const value: WorldContextValue = {
        connectors,
    };

    return (
        <StarknetConfig connectors={connectors}>
            <WorldContext.Provider value={value}>
                <RPCProvider worldAddress={worldAddress} rpcUrl={rpcUrl}>
                    <WebSocketProvider ws={ws}>
                        {children}
                    </WebSocketProvider>
                </RPCProvider>
            </WorldContext.Provider>
        </StarknetConfig>
    );
};

export function useWorldContext(): WorldContextValue {
    const context = useContext(WorldContext);
    if (!context) {
        throw new Error('useWorldContext must be used within a WorldProvider');
    }
    return context;
}