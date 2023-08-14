import { RpcProvider, Account, num, Call, InvokeFunctionResponse } from "starknet";
import { Provider } from "./provider";
import { Query, WorldEntryPoints } from "../types";
import { strTofelt252Felt } from '../utils'
import { LOCAL_KATANA } from '../constants';

export class RPCProvider extends Provider {
    public provider: RpcProvider;

    constructor(world_address: string, url: string = LOCAL_KATANA) {
        super(world_address);
        this.provider = new RpcProvider({
            nodeUrl: url,
        });
    }

    public async entity(component: string, query: Query, offset: number = 0, length: number = 0): Promise<Array<bigint>> {

        const call: Call = {
            entrypoint: WorldEntryPoints.get, // "entity"
            contractAddress: this.getWorldAddress(),
            calldata: [
                strTofelt252Felt(component),
                query.address_domain,
                query.keys.length,
                ...query.keys as any,
                offset,
                length
            ]
        }

        try {
            const response = await this.provider.callContract(call);

            return response.result as unknown as Array<bigint>;
        } catch (error) {
            throw error;
        }
    }

    public async entities(component: string, length: number): Promise<Array<bigint>> {

        const call: Call = {
            entrypoint: WorldEntryPoints.entities,
            contractAddress: this.getWorldAddress(),
            calldata: [strTofelt252Felt(component), length]
        }

        try {
            const response = await this.provider.callContract(call);

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
            const response = await this.provider.callContract(call);

            return response.result as unknown as bigint;
        } catch (error) {
            throw error;
        }
    }

    public async execute(account: Account, system: string, call_data: num.BigNumberish[]): Promise<InvokeFunctionResponse> {

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