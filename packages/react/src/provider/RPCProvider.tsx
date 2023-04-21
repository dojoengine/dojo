import React, { createContext, useContext, useState, useEffect } from 'react';
import { Providers } from '@dojoengine/core';

export interface DojoContext {
  worldAddress?: string;
  rpcProvider?: Providers.RPCProvider;
}

const DOJO_INITIAL_STATE: DojoContext = {
  worldAddress: undefined,
};

const RpcContext = createContext<DojoContext>(DOJO_INITIAL_STATE);

interface DojoConfigProps {
  worldAddress: string;
  rpcUrl: string | "https://starknet-goerli.cartridge.gg/";
  children: React.ReactNode;
}

export function RPCProvider({
  children,
  worldAddress,
  rpcUrl,
}: DojoConfigProps) {
  const [rpcProvider, setRpcProvider] = useState<Providers.RPCProvider>();

  useEffect(() => {
    const newRpcProvider = new Providers.RPCProvider(worldAddress, rpcUrl);
    setRpcProvider(newRpcProvider);
  }, [worldAddress, rpcUrl]);

  return (
    <RpcContext.Provider value={{ worldAddress, rpcProvider }}>
      {children}
    </RpcContext.Provider>
  );
};

export function useRPCContext(): DojoContext {
  const context = useContext(RpcContext);
  if (!context) {
    throw new Error('RPCContext must be used within a DojoConfig');
  }
  return context;
}