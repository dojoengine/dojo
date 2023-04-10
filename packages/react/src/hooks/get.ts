import { useCallback } from 'react';
import { useDojoContext } from '../provider';
import { Query } from 'dojo-core/dist/types';
import { Store } from 'dojo-core';

// todo: this should really be components and not entities

// key should be from world setup, and should be an optional trigger to rerender
export function useDojoEntity<T>({
  key,
  parser,
}: {
  key: any;
  parser: (data: any) => T | undefined;
}) {
  // -- Context -- //
  const { rpcProvider } = useDojoContext();

  // -- Store -- //
  const store = Store.EntityStore;

  // -- Callbacks -- //
  const getEntityCallback = useCallback(
    async (
      component: bigint,
      query: Query,
      offset: number,
      length: number
    ) => {
      await getEntity(
        store,
        rpcProvider,
        component,
        query,
        offset,
        length
      );
    },
    [rpcProvider, key, parser]
  );

  const setEntityCallback = useCallback(
    async (
      optimistic: boolean,
      value: bigint[],
      component: bigint,
      query: Query,
      offset: number,
      length: number
    ) => {
      await setEntity(
        store,
        rpcProvider,
        optimistic,
        value,
        component,
        query,
        offset,
        length
      );
    },
    [rpcProvider, key, parser]
  );

  return {
    entity: parser(store.getState()),
    getEntity: getEntityCallback,
    setEntity: setEntityCallback,
  };
}

// we should pass in providers here

// todo - this should know the component and fill in the gaps
export async function setEntity<T>(
  store: any, // TODO: Types
  rpcProvider: any,
  optimistic: boolean,
  value: bigint[],
  component: bigint,
  query: Query,
  offset: number,
  length: number
) {
  // TODO: This is very incomplete
  // Set the entity immediately in the store
  // Idea here was to optimistically update the state in zustand to reflect the users input
  // restrictions in the client logic can control auth here.
  // From a User POV, most of the time they just want to see the reflection in the client of what they have done, this gives a client
  // representation of it.
  // this could be replaced in the future with the state diff
  if (optimistic) store.setState({ entity: value });

  // execute here
  if (rpcProvider) {
    await rpcProvider.updateEntity(component, query, offset, length);
    // Trigger getEntity to fetch the updated entity or 

    // Fetch entity 
    await getEntity(
      store,
      rpcProvider,
      component,
      query,
      offset,
      length
    );
  }
}

// we should pass in providers here
export async function getEntity<T>(
  store: any, // todo: fix types
  rpcProvider: any,
  component: bigint,
  query: Query,
  offset: number,
  length: number
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