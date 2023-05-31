import { useCallback, useState } from 'react';
import { useRPCContext } from '../provider';
import {
    useContractWrite,
    Call
} from "@starknet-react/core";
import { Account, ec, Provider, stark, number } from "starknet";


const provider = new Provider({ sequencer: { network: 'mainnet-alpha', baseUrl: "http://127.0.0.1:5050" }, rpc: { nodeUrl: "http://127.0.0.1:5050" } })

// this could be moved to the core...

// TODO: expose calls as an array to client so users can build a queue of calls
// TODO: loading

const account = "0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea"

const privateKey = "0x07230b49615d175307d580c33d6fda61fc7b9aec91df0f5c1a5ebe3b8cbfee02"

export function useSystem<T>({
    key,
}: {
    key: string;
}) {
    const [calls, setCalls] = useState<Call[]>([])
    const { write } = useContractWrite({ calls })

    const { rpcProvider, worldAddress } = useRPCContext()

    const execute = useCallback(
        async (
            call_data: number.BigNumberish[],
            system: string
        ) => {

            console.log("Execute: ", call_data, system)

            const starkKeyPair = ec.getKeyPair(privateKey);

            let stark_account;



            if (rpcProvider?.sequencerProvider) stark_account = new Account(provider, account, starkKeyPair)

            console.log(rpcProvider?.sequencerProvider)

            let call_data_obj = call_data.reduce((obj: any, item, index) => {
                obj[index] = item;
                return obj;
            }, {});

            try {
                const nonce = await stark_account?.getNonce()
                const call = await stark_account?.execute(
                    {
                        contractAddress: worldAddress || "",
                        entrypoint: 'execute',
                        calldata: stark.compileCalldata({
                            system,
                            ...call_data_obj
                        })
                    },
                    undefined,
                    {
                        nonce: nonce,
                        maxFee: 0
                    }
                );

                console.log(call)

            } catch (e) {
                console.log(e)
            }


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
