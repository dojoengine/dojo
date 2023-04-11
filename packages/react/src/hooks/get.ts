import { useCallback } from 'react';
import { useDojoContext } from '../provider';
import { Query } from 'dojo-core/dist/types';
import { Store } from 'dojo-core';

// key should be from world setup, and should be an optional trigger to rerender
export function useComponent<T>({
  key,
  parser,
  optimistic = false,
}: {
  key: any;
  parser: (data: any) => T | undefined;
  optimistic: boolean;
}) {

  // -- Context -- //
  const { rpcProvider } = useDojoContext();

  // -- Store -- //
  const store = Store.ComponentStore;

  // -- Callbacks -- //
  const getComponentCallback = useCallback(
    async (
      component: string,
      query: Query
    ) => {
      await getEntity(
        store,
        rpcProvider,
        component,
        query
      );
    },
    [rpcProvider, key, parser]
  );

  return {
    entity: parser(store.getState()),
    getEntity: getComponentCallback
  };
}

// we should pass in providers here to make it modular
export async function getEntity<T>(
  store: any, // todo: fix types
  rpcProvider: any,
  component: string,
  query: Query,
  offset: number = 0,
  length: number = 0
) {
  if (rpcProvider) {
    const fetchedEntity = await rpcProvider.entity(
      component,
      query,
      offset,
      length
    );
    store.setState({ entity: fetchedEntity });
  } else {
    store.setState({ entity: [] });
  }
}