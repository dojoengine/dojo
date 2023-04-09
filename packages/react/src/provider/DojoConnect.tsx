import React, { createContext, useContext, useState, useEffect } from 'react';
import { Providers } from 'dojo-core';

export interface DojoContext {
  worldAddress?: string;
  rpcProvider?: Providers.RPCProvider;
}

const DOJO_INITIAL_STATE: DojoContext = {
  worldAddress: undefined // we can add a default world address here
};

const RpcContext = createContext<DojoContext>(DOJO_INITIAL_STATE);

interface DojoConfigProps {
  worldAddress: string;
  rpcUrl?: string;
  children: React.ReactNode;
}

export const DojoConfig: React.FC<DojoConfigProps> = ({
  children,
  worldAddress,
  rpcUrl,
}: any) => {
  const [rpcProvider, setRpcProvider] = useState<Providers.RPCProvider>();

  useEffect(() => {
    const newRpcProvider = new Providers.RPCProvider(worldAddress, rpcUrl);
    setRpcProvider(newRpcProvider);
  }, [worldAddress, rpcUrl]);

  return (
    <RpcContext.Provider value={{ rpcProvider }}>
      {children}
    </RpcContext.Provider>
  );
};

export function useDojoContext(): DojoContext {
  const context = useContext(RpcContext);
  if (!context) {
    throw new Error('useDojoContext must be used within a DojoConfig');
  }
  return context;
}