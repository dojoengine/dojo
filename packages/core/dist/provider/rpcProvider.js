"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RPCProvider = void 0;
const httpProvider_1 = require("./httpProvider");
const rpcError_1 = require("./error/rpcError");
class RPCProvider extends httpProvider_1.HttpProvider {
    constructor(url) {
        super();
        this.url = url;
        this.requestId = 1;
    }
    async call(method, params) {
        const rpcRequest = {
            jsonrpc: "2.0",
            id: this.requestId++,
            method,
            params,
        };
        const requestOptions = {
            method: "POST",
            url: this.url,
            data: rpcRequest,
            headers: { "Content-Type": "application/json" },
        };
        try {
            const response = await this.send(requestOptions);
            if (response.error) {
                throw new rpcError_1.RPCError(response.error.message, response.error.code, response.error.data);
            }
            return response.result;
        }
        catch (error) {
            this.emit("error", error);
            throw error;
        }
    }
}
exports.RPCProvider = RPCProvider;
