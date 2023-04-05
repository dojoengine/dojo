import { HttpProvider } from "./httpProvider";
import { RPCError } from "./error/rpcError";
import { IRequestOptions } from "./types";

interface IRPCRequest {
    jsonrpc: string;
    id: number;
    method: string;
    params?: any[];
}

export class RPCProvider extends HttpProvider {
    private url: string;
    private requestId: number;

    constructor(url: string) {
        super();
        this.url = url;
        this.requestId = 1;
    }

    public async call(method: string, params?: any[]): Promise<any> {
        const rpcRequest: IRPCRequest = {
            jsonrpc: "2.0",
            id: this.requestId++,
            method,
            params,
        };

        const requestOptions: IRequestOptions = {
            method: "POST",
            url: this.url,
            data: rpcRequest,
            headers: { "Content-Type": "application/json" },
        };

        try {
            const response = await this.send(requestOptions);
            if (response.error) {
                throw new RPCError(response.error.message, response.error.code, response.error.data);
            }
            return response.result;
        } catch (error) {
            this.emit("error", error);
            throw error;
        }
    }
}