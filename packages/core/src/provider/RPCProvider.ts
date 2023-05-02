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

    // fetches a component of an entity
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

    // fetches multiple components of an entity
    public async constructEntity(parameters: Array<{ component: string; query: Query; offset: number; length: number }>): Promise<{ [key: string]: Array<bigint> }> {
        const responseObj: { [key: string]: Array<bigint> } = {};

        for (const param of parameters) {
            const { component, query, offset, length } = param;
            try {
                const response = await this.entity(component, query, offset, length);
                responseObj[component] = response;
            } catch (error) {
                this.log("error", `Fetch multiple entities failed for component ${component}: ${error}`);
                this.emit("error", error);
                throw error;
            }
        }

        this.log("info", `Fetch multiple entities successful: ${JSON.stringify(responseObj)}`);
        return responseObj;
    }
}