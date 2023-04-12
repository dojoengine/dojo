import React, { createContext, useContext, useState, useEffect } from 'react';
import { Providers } from '@dojoengine/core';

export interface DojoContext {
  worldAddress?: string;
  rpcProvider?: Providers.RPCProvider;
  components: { [id: string]: any };
  registerComponent: (components: { id: string; component: any }[]) => void;
  unregisterComponent: (id: string) => void;
}

const DOJO_INITIAL_STATE: DojoContext = {
  worldAddress: undefined,
  components: {},
  registerComponent: () => { },
  unregisterComponent: () => { },
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

  // TODO: Do we also move this to zustand?
  const [components, setComponents] = useState<{ [id: string]: any }>({});

  // --- Component Registry --- //
  const registerComponent = (newComponents: { id: string; component: any }[]) => {
    setComponents((prevComponents) => {
      const updatedComponents = { ...prevComponents };
      newComponents.forEach(({ id, component }) => {
        updatedComponents[id] = component;
      });
      return updatedComponents;
    });
  };

  const unregisterComponent = (id: string) => {
    setComponents((prevComponents) => {
      const { [id]: _, ...rest } = prevComponents;
      return rest;
    });
  };

  return (
    <RpcContext.Provider value={{ worldAddress, rpcProvider, components, registerComponent, unregisterComponent }}>
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