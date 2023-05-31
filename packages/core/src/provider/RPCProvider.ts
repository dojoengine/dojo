import { RpcProvider, Provider as StarknetProvider, Account, stark, number, Call } from "starknet";
import { Provider } from "./provider";
import { Query, WorldEntryPoints } from "../types";
import * as microstarknet from 'micro-starknet';
import { strToShortStringFelt } from '../utils'
import BN__default from 'bn.js';

export class RPCProvider extends Provider {
    public provider: RpcProvider;
    public sequencerProvider: StarknetProvider;
    private loggingEnabled: boolean;

    constructor(world_address: string, url: string, loggingEnabled = false) {
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

    public async entity(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>> {

        const poseidon: any = microstarknet.poseidonHashMany(query.keys)

        const call: Call = {
            entrypoint: WorldEntryPoints.get, // "entity"
            contractAddress: this.getWorldAddress(),
            calldata: [
                strToShortStringFelt(component),
                query.address_domain,
                query.partition,
                query.keys.length,
                ...query.keys,
                poseidon,
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
            calldata: [strToShortStringFelt(component), partition]
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
            calldata: [strToShortStringFelt(name)]
        }

        try {
            const response = await this.sequencerProvider.callContract(call);

            return response.result as unknown as bigint;
        } catch (error) {
            throw error;
        }
    }

    public async execute(account: Account, system: string, call_data: number.BigNumberish[]): Promise<Array<bigint>> {

        let call_data_obj = call_data.reduce((obj: any, item, index) => {
            obj[index] = item;
            return obj;
        }, {});

        try {
            const nonce = await account?.getNonce()
            const call = await account?.execute(
                {
                    contractAddress: this.getWorldAddress() || "",
                    entrypoint: WorldEntryPoints.execute,
                    calldata: stark.compileCalldata({
                        name: strToShortStringFelt(system),
                        ...call_data_obj
                    })
                },
                undefined,
                {
                    nonce: nonce,
                    maxFee: 0 // TODO: Update
                }
            );
            return call as unknown as Array<bigint>;
        } catch (error) {
            throw error;
        }
    }
}