import { RpcProvider } from "starknet";
import { Call } from "starknet";
import { Provider } from "./provider";
import { Query, WorldEntryPoints } from "../types";
import logger from "../logging/logger";

export class RPCProvider extends Provider {
    private provider: RpcProvider;
    private loggingEnabled: boolean;

    constructor(world_address: string, url: string, loggingEnabled = false) {
        super(world_address);
        this.provider = new RpcProvider({
            nodeUrl: url,
        });
        this.loggingEnabled = loggingEnabled;
    }

    private log(level: string, message: string) {
        if (this.loggingEnabled) {
            logger.log(level, message);
        }
    }

    public async entity(component: string, query: Query, offset: number, length: number): Promise<Array<bigint>> {

        const call_data = [component, query.partition, ...query.keys, offset, length]

        const call: Call = {
            entrypoint: WorldEntryPoints.get,
            contractAddress: this.getWorldAddress(),
            calldata: call_data
        }

        try {
            const response = await this.provider.callContract(call);
            this.log("info", `Entity call successful: ${JSON.stringify(response)}`);
            return response.result as unknown as Array<bigint>;
        } catch (error) {
            this.log("error", `Entity call failed: ${error}`);
            this.emit("error", error);
            throw error;
        }
    }
}