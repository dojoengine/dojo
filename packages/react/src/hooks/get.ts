import { useCallback } from 'react';
import { useDojoContext } from '../provider';
import { Query } from 'dojo-core/dist/types';
import { Store } from 'dojo-core';

export function useDojoEntity<T>({
  key,
  parser,
}: {
  key: any;
  parser: (data: any) => T | undefined;
}) {

  // -- Store -- //
  const store = Store.EntityStore;

  // -- Context -- //
  const { rpcProvider } = useDojoContext();

  // -- Callbacks -- //
  const getEntity = useCallback(
    async (
      component: bigint,
      query: Query,
      offset: number,
      length: number
    ) => {
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
    },
    [rpcProvider, key]
  );



  console.log("useDojoEntity", store.getState());

  return { entity: parser(store.getState()), getEntity, setEntity: store.setState };
}
