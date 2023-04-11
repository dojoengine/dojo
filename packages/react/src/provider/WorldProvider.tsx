import React, { createContext, useContext, ReactNode } from 'react';
import ControllerConnector from "@cartridge/connector";
import {
    InjectedConnector,
    StarknetProvider,
    useConnectors,
} from "@starknet-react/core";
import { DojoConfig } from './DojoConnect';

export interface WorldContextValue {
    connectors: any[];
    connect: (connector: any) => void;
    // Add any properties or functions you want to expose in this context
}

const WorldContext = createContext<WorldContextValue | null>(null);

interface WorldProviderProps {
    worldAddress: string;
    rpcUrl?: string;
    children: ReactNode;
}

const rpcUrl = "https://starknet-goerli.cartridge.gg/";

// might need to pass this in as a prop
const controllerConnector = new ControllerConnector([
    {
        target: "0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a",
        method: "execute",
    },
]);

const argentConnector = new InjectedConnector({
    options: {
        id: "argentX",
    },
});

const connectors = [controllerConnector as any, argentConnector];

export const WorldProvider: React.FC<WorldProviderProps> = ({ worldAddress, rpcUrl, children }) => {
    const { connect } = useConnectors()
    const value: WorldContextValue = {
        connectors,
        connect
    };
    return (
        <WorldContext.Provider value={value}>
            <StarknetProvider connectors={connectors}>
                <DojoConfig worldAddress={worldAddress} rpcUrl={rpcUrl}>
                    {children}
                </DojoConfig>
            </StarknetProvider>
        </WorldContext.Provider>
    );
};

export function useWorldContext(): WorldContextValue {
    const context = useContext(WorldContext);
    if (!context) {
        throw new Error('useWorldContext must be used within a WorldProvider');
    }
    return context;
}