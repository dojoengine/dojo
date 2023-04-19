import { useCallback, useState } from 'react';
import { useDojoContext } from '../provider';
import {
    useContractWrite,
    Call
} from "@starknet-react/core";

// this could be moved to the core...

// todo expose calls as an array to client so users can build a queue of calls
export function useSystem<T>({
    key,
}: {
    key: string;
}) {
    const [calls, setCalls] = useState<Call[]>([])
    const { write } = useContractWrite({ calls })
    const { worldAddress } = useDojoContext();

    const execute = useCallback(
        async (
            call_data: bigint[],
            system: string
        ) => {

            console.log("Execute: ", call_data, system)

            // const call: Call = {
            //     entrypoint: "execute",
            //     contractAddress: worldAddress || "",
            //     calldata: [system, ...call_data]
            // }

            // setCalls([call])
            // write()
        },
        [key]
    );

    return {
        execute
    };
}
