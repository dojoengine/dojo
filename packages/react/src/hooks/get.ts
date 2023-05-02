import { useCallback } from 'react';
import { useDojoContext } from '../provider';
import { Query } from '@dojoengine/core/dist/types';

// key should be from world setup, and should be an optional trigger to rerender
export function useComponent<T>({
  key,
  parser,
  store
}: {
  key: string;
  parser: (data: any) => T | undefined;
  store: any;
}) {
  const { rpcProvider } = useDojoContext();


  const getComponentCallback = useCallback(
    async (
      component: string,
      query: Query
    ) => {

      // todo pass in different providers here
      await getComponent(
        store,
        rpcProvider,
        component,
        query
      );
    },
    [rpcProvider, key, parser]
  );

  console.log("useComponent: ", store.getState())

  return {
    component: parser(store.getState().value),
    getComponent: getComponentCallback
  };
}

// we should pass in providers here to make it modular
export async function getComponent<T>(
  store: any, // todo: fix types
  rpcProvider: any,
  component: string,
  query: Query,
  offset: number = 0,
  length: number = 0
) {
  if (rpcProvider) {
    const componentState = await rpcProvider.entity(
      component,
      query,
      offset,
      length
    );
    store.setState({ value: componentState });
  }
}

