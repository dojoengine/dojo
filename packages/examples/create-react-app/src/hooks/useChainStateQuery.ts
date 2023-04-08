import { useEffect, useState } from "react";
import { Entity, QueryResult } from "../types";


// Fetch the current state of an entity from the chain
export const useChainStateQuery = <T>(promises: Promise<T>[], entity_id: number, entity_type: Entity): QueryResult<T> => {
    const [state, setState] = useState<T>(undefined as unknown as T);
    const [loading, setLoading] = useState<boolean>(true);
    const [error, setError] = useState<Error | undefined>(undefined);

    useEffect(() => {
        const fetchData = async () => {
            try {
                const results = await Promise.all(promises);
                setState(results as unknown as T);
                setLoading(false);
            } catch (err) {
                // setError(err);
                setLoading(false);
            }
        };

        fetchData();
    }, [promises]);

    return { data: state as unknown as T, loading: loading, error: undefined };
}

