import { useCallback, useState } from 'react';
import { useDojoContext } from '../provider';
import { Store } from 'dojo-core';
import {
    useContractWrite,
    Call
} from "@starknet-react/core";

// this could be moved to the core...

// todo expose calls as an array to client so users can build a queue of calls
export function useSystem<T>({
    key,
}: {
    key: any;
}) {

    const [calls, setCalls] = useState<Call[]>([])

    const { write } = useContractWrite({ calls })

    // -- Context -- //
    const { rpcProvider, worldAddress } = useDojoContext();

    // -- Store -- //
    const store = Store.ComponentStore;

    const execute = useCallback(
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

            // we need type validation here to make sure the value is correct
            // in future we can use state diff
            if (optimistic) store.setState({ value: value });
        },
        [rpcProvider, key]
    );

    return {
        execute
    };
}
