import { RpcProvider } from "starknet";
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
    public async entity(component: bigint, query: Query, offset: number, length: number): Promise<Array<bigint>> {

        // TODO: Can we construct the offset and length from the manifest?
        const call: Call = {
            entrypoint: WorldEntryPoints.get,
            contractAddress: this.getWorldAddress(),
            calldata: [component.toString(), query.partition, ...query.keys, offset, length]
        }

        try {
            const response = await this.provider.callContract(call)
            return response.result as unknown as Array<bigint>;
        } catch (error) {
            this.emit("error", error);
            throw error;
        }
    }
}