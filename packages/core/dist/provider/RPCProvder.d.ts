import { HttpProvider } from "./HttpProvider";
export declare class RPCProvider extends HttpProvider {
    private url;
    private requestId;
    constructor(url: string);
    call(method: string, params?: any[]): Promise<any>;
}
