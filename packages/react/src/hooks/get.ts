import { useCallback, useMemo, useState } from 'react';
import { useDojoContext } from '../provider';
import { Query } from 'dojo-core/dist/types';
import { Store } from 'dojo-core';
import {
  useContractWrite,
  Call
} from "@starknet-react/core";
// import { Call } from 'starknet';

// todo: this should really be components and not entities

// key should be from world setup, and should be an optional trigger to rerender
export function useDojoEntity<T>({
  key,
  parser,
  optimistic = false,
}: {
  key: any;
  parser: (data: any) => T | undefined;
  optimistic: boolean;
}) {

  const [calls, setCalls] = useState<Call[]>([])

  const { write } = useContractWrite({ calls })

  // -- Context -- //
  const { rpcProvider, worldAddress } = useDojoContext();

  // -- Store -- //
  const store = Store.EntityStore;

  // -- Callbacks -- //
  const getEntityCallback = useCallback(
    async (
      component: string,
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
      value: bigint[],
      component: string,
      optimistic: boolean = false,
    ) => {

      const call: Call = {
        entrypoint: "execute",
        contractAddress: worldAddress || "",
        calldata: [component, ...value]
      }

      setCalls([call])

      write()

      if (optimistic) store.setState({ entity: value });
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

// TODO: This is very incomplete
// Set the entity immediately in the store
// Idea here was to optimistically update the state in zustand to reflect the users input
// restrictions in the client logic can control auth here.
// From a User POV, most of the time they just want to see the reflection in the client of what they have done, this gives a client
// representation of it.
// this could be replaced in the future with the state diff
export async function setEntity<T>(
  worldAddress: string = "",
  store: any, // TODO: Types
  rpcProvider: any,
  value: bigint[],
  component: string,
  optimistic: boolean,
) {

  // // TODO: Get world types
  // const call: Call = {
  //   entrypoint: "execute",
  //   contractAddress: worldAddress,
  //   calldata: [component, ...value]
  // }

  // const { execute } = useStarknetExecute({ calls: [call] })

  // execute()

  // if (optimistic) store.setState({ entity: value });

}

// we should pass in providers here
export async function getEntity<T>(
  store: any, // todo: fix types
  rpcProvider: any,
  component: string,
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