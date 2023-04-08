import { useCallback, useState } from "react";
import { useDojoContext } from "../provider";
import { Query } from "../../../core/src/types";


// -- Key is used to force a re-render when the key changes
// -- Parser is used to convert the data from the provider into the correct type

export function useDojoEntity<T>({ key, parser }: { key: any; parser: (data: any) => T }) {
  const [entity, setEntity] = useState<T | null>(null);
  const { rpcProvider } = useDojoContext();

  // could get the interface here and pass into function
  const getEntity = useCallback(
    async (component: bigint, query: Query, offset: number, length: number) => {
      if (rpcProvider) {
        const fetchedEntity = await rpcProvider.entity(component, query, offset, length);
        setEntity(parser(fetchedEntity));
      } else {
        setEntity(null);
      }
    },
    [rpcProvider, key]
  );

  return { entity, getEntity };
}