import React, { createContext, useContext, ReactNode } from 'react';
import ControllerConnector from "@cartridge/connector";
import {
    InjectedConnector,
    StarknetProvider,
    useConnectors,
} from "@starknet-react/core";
import { DojoConfig } from './DojoConnect';


// World Provider
// Contains an entire world setup for a react app. 
// It also exposes chain context like block number 
// and other network specifci information to use 
// in a client.


export interface WorldContextValue {
    connectors: any[];
    connect: (connector: any) => void;
}

const WorldContext = createContext<WorldContextValue | null>(null);

interface WorldProviderProps {
    worldAddress: string;
    rpcUrl?: string;
    children: ReactNode;
    connectors: any[];
}

const rpcUrl = "https://starknet-goerli.cartridge.gg/";

export const WorldProvider: React.FC<WorldProviderProps> = ({ worldAddress, rpcUrl, children, connectors }) => {
    const { connect } = useConnectors()
    const value: WorldContextValue = {
        connectors,
        connect
    };
    return (
        <StarknetProvider connectors={connectors}>
            <WorldContext.Provider value={value}>
                <DojoConfig worldAddress={worldAddress} rpcUrl={rpcUrl}>
                    {children}
                </DojoConfig>
            </WorldContext.Provider>
        </StarknetProvider>
    );
};

export function useWorldContext(): WorldContextValue {
    const context = useContext(WorldContext);
    if (!context) {
        throw new Error('useWorldContext must be used within a WorldProvider');
    }
    return context;
}