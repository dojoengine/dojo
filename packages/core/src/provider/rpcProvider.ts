import { RpcProvider, number } from "starknet";
import { Call } from "starknet";
import { Provider } from "./provider";
import { Query, WorldEntryPoints } from "../types";

export class RPCProvider extends Provider {
    private provider: RpcProvider

    constructor(world_address: string, url: string) {
        super(world_address);
        this.provider = new RpcProvider({
            nodeUrl: url,
        })
    }

    // TODO: Add interface shape
    public async entity(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>> {

        const call_data = [component, query.partition, ...query.keys, offset, length]

        console.log("call", call_data)

        const call: Call = {
            entrypoint: WorldEntryPoints.get,
            contractAddress: this.getWorldAddress(),
            calldata: call_data
        }

        try {
            const response = await this.provider.callContract(call)
            return response.result as unknown as Array<bigint>;
        } catch (error) {
            this.emit("error", error);
            throw error;
        }
    }

    // public async execute(name: bigint, execute_calldata: string[]): Promise<Array<bigint>> {


    //     const call: Call = {
    //         entrypoint: WorldEntryPoints.execute,
    //         contractAddress: this.getWorldAddress(),
    //         calldata: [name.toString(), ...execute_calldata]
    //     }

    //     try {
    //         const response = await this.provider.callContract(call)
    //         return response.result as unknown as Array<bigint>;
    //     } catch (error) {
    //         this.emit("error", error);
    //         throw error;
    //     }
    // }
}
