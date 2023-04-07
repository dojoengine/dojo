"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RPCProvider = void 0;
const starknet_1 = require("starknet");
const provider_1 = require("./provider");
const types_1 = require("../types");
class RPCProvider extends provider_1.Provider {
    constructor(world_address, url) {
        super(world_address);
        this.provider = new starknet_1.RpcProvider({
            nodeUrl: url,
        });
    }
    async set_entity(component, key, offset, value, calldata) {
        return;
    }
    async get_component(component, entity_id, offset, length) {
        // TODO: Can we construct the offset and length from the manifest?
        const call = {
            entrypoint: types_1.WorldEntryPoints.get,
            contractAddress: this.getWorldAddress(),
            calldata: [component, entity_id, offset, length]
        };
        try {
            const response = await this.provider.callContract(call);
            return response.result;
        }
        catch (error) {
            this.emit("error", error);
            throw error;
        }
    }
    async get_entity() {
        return [];
    }
    async get_entities() {
        return [];
    }
}
exports.RPCProvider = RPCProvider;
