import { useState } from "react";
import { Entity, EntityData, QueryResult } from "../types";

// Fetch the history of a given entity from indexer NOT the chain - this in optional
export const useHistoryQuery = <T>(entity_id: number, entity_type: Entity): QueryResult<T> => {
    const [state, setState] = useState<T>(undefined as unknown as T);
    const [loading, setLoading] = useState<boolean>(true);

    // Recurse through state and constuct

    // save into state

    // poll every second
    const result: EntityData = {
        entityId: entity_id,
        entityType: entity_type,
    };

    return { data: result as unknown as T, loading: loading, error: undefined };
}