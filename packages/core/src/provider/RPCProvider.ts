import { RpcProvider, Provider as StarknetProvider, Account, stark, number, Call, InvokeFunctionResponse } from "starknet";
import { Provider } from "./provider";
import { Query, WorldEntryPoints } from "../types";
import { strTofelt252Felt } from '../utils'
import { LOCAL_TORII } from '../constants';

export class RPCProvider extends Provider {
    public provider: RpcProvider;
    public sequencerProvider: StarknetProvider;
    private loggingEnabled: boolean;

    constructor(world_address: string, url: string = LOCAL_TORII, loggingEnabled = false) {
        super(world_address);
        this.provider = new RpcProvider({
            nodeUrl: url,
        });

        // have to use this provider with Starknet.js
        this.sequencerProvider = new StarknetProvider({
            sequencer: {
                // TODO: change name to KATANA
                network: 'mainnet-alpha',
                baseUrl: url
            },
            rpc: {
                nodeUrl: url
            }
        })
        this.loggingEnabled = loggingEnabled;
    }

    public async entity(component: string, query: Query, offset: number = 0, length: number = 0): Promise<Array<bigint>> {

        const call: Call = {
            entrypoint: WorldEntryPoints.get, // "entity"
            contractAddress: this.getWorldAddress(),
            calldata: [
                strTofelt252Felt(component),
                query.address_domain,
                query.partition,
                query.keys.length,
                ...query.keys as any,
                offset,
                length
            ]
        }

        console.log(call)

        try {
            const response = await this.sequencerProvider.callContract(call);

            return response.result as unknown as Array<bigint>;
        } catch (error) {
            throw error;
        }
    }

    public async entities(component: string, partition: string): Promise<Array<bigint>> {

        const call: Call = {
            entrypoint: WorldEntryPoints.entities,
            contractAddress: this.getWorldAddress(),
            calldata: [strTofelt252Felt(component), partition]
        }

        console.log(call)

        try {
            const response = await this.sequencerProvider.callContract(call);

            return response.result as unknown as Array<bigint>;
        } catch (error) {
            throw error;
        }
    }

    public async component(name: string): Promise<bigint> {

        const call: Call = {
            entrypoint: WorldEntryPoints.component,
            contractAddress: this.getWorldAddress(),
            calldata: [strTofelt252Felt(name)]
        }

        try {
            const response = await this.sequencerProvider.callContract(call);

            return response.result as unknown as bigint;
        } catch (error) {
            throw error;
        }
    }

    public async execute(account: Account, system: string, call_data: number.BigNumberish[]): Promise<InvokeFunctionResponse> {

        let execute_calldata = call_data.map((c) => c.toString());

        try {
            const nonce = await account?.getNonce()
            const call = await account?.execute(
                {
                    contractAddress: this.getWorldAddress() || "",
                    entrypoint: WorldEntryPoints.execute,
                    calldata: stark.compileCalldata({
                        name: strTofelt252Felt(system),
                        execute_calldata
                    })
                },
                undefined,
                {
                    nonce: nonce,
                    maxFee: 0 // TODO: Update
                }
            );
            return call;
        } catch (error) {
            throw error;
        }
    }
}