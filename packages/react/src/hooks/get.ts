import { useCallback, useState } from "react";
import { useDojoContext } from "../provider";
import { Query } from "../../../core/src/types";

export function useDojoEntity({key}: any) {
    const [entity, setEntity] = useState<bigint[] | null>(null);
    const { rpcProvider } = useDojoContext();
    
    // could get the interface here and pass into function
    const getEntity = useCallback(
      async (component: bigint, query: Query, offset: number, length: number) => {
        if (rpcProvider) {
          const fetchedEntity = await rpcProvider.entity(component, query, offset, length);
          setEntity(fetchedEntity);
        } else {
          setEntity(null);
        }
      },
      [rpcProvider, key]
    );
  
    return { entity, getEntity };
  }