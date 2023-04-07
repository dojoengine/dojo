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

    public async set_entity(component: number,
        query: Query,
        offset: number,
        value: number[],
        calldata?: any[]): Promise<any> {
        return;
    }

    public async get_component(component: string, entity_id: string, offset: string, length: string): Promise<any> {

        // TODO: Can we construct the offset and length from the manifest?
        const call: Call = {
            entrypoint: WorldEntryPoints.get,
            contractAddress: this.getWorldAddress(),
            calldata: [component, entity_id, offset, length]
        }

        try {
            const response = await this.provider.callContract(call)
            return response.result;
        } catch (error) {
            this.emit("error", error);
            throw error;
        }
    }

    public async get_entity(): Promise<any[]> {
        return [];
    }
    public async get_entities(): Promise<any[]> {
        return [];
    }
}