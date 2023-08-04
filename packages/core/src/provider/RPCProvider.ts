import { RpcProvider, Provider as StarknetProvider, Account, number, Call, InvokeFunctionResponse } from "starknet";
import { Provider } from "./provider";
import { Query, WorldEntryPoints } from "../types";
import { strTofelt252Felt } from '../utils'
import { LOCAL_KATANA } from '../constants';

export class RPCProvider extends Provider {
    public provider: RpcProvider;
    public sequencerProvider: StarknetProvider;
    private loggingEnabled: boolean;

    constructor(world_address: string, url: string = LOCAL_KATANA, loggingEnabled = false) {
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

        try {
            const response = await this.sequencerProvider.callContract(call);

            return response.result as unknown as Array<bigint>;
        } catch (error) {
            throw error;
        }
    }

    public async entities(component: string, partition: string, length: number): Promise<Array<bigint>> {

        const call: Call = {
            entrypoint: WorldEntryPoints.entities,
            contractAddress: this.getWorldAddress(),
            calldata: [strTofelt252Felt(component), partition, length]
        }

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

        try {
            const nonce = await account?.getNonce()
            const call = await account?.execute(
                {
                    contractAddress: this.getWorldAddress() || "",
                    entrypoint: WorldEntryPoints.execute,
                    calldata: [strTofelt252Felt(system), call_data.length, ...call_data]
                },
                undefined,
                {
                    nonce,
                    maxFee: 0 // TODO: Update
                }
            );
            return call;
        } catch (error) {
            throw error;
        }
    }
}